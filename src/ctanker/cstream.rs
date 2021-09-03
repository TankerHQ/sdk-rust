//! Tanker streaming APIs take a stream as input and outputs another stream.
//! The returned stream is TankerStream, it stores the user's stream.
//!
//! The UserStream is not necessarily Send, so we can't read it on Tanker's thread.
//! Also, it only exposes poll() things, and we don't have a Context there to
//! call any poll function. This is why the stream is read in the poll_read function
//! of TankerStream.
//!
//! We need to forward a read request from Tanker to the poll_read of TankerStream.
//! To do that, we have a channel between the two. On Tanker's thread, we send a
//! ReadOperation which contains the Tanker buffer and handle to declare the end of
//! the read. We call the waker that has been previously set by poll_read to
//! trigger the read.
//! We need to pass the waker around because there's no way to pass the context to
//! the Receiver so that it wakes us up by itself.
//!
//! ```text
//! TankerStream::poll_read -> tanker_stream_read -> ::read_underlying_stream
//!                  ^                                       |
//!                  '----------- ReadOperation -------------'
//! ```

use super::bindings::*;
use crate::ctanker::*;
use crate::error::Error;

use ::core::pin::Pin;
use async_channel::{bounded, Receiver, Sender, TryRecvError};
use futures::executor::block_on;
use futures::future::{select, Either};
use futures::io::{AsyncRead, AsyncReadExt};
use futures::ready;
use futures::task::{Context, Poll, Waker};
use futures::FutureExt;
use std::cmp::min;
use std::future::Future;
use std::sync::Mutex;

#[derive(Debug, Clone)]
struct ReadOperation {
    buffer: *mut u8,
    size: i64,
    operation: *mut tanker_stream_read_operation_t,
}

struct SenderBundle {
    sender: Sender<ReadOperation>,
    waker: Mutex<Option<Waker>>,
}

// Same chunk size as Tanker to avoid useless back and forth.
const STREAM_CHUNK_SIZE: usize = 1024 * 1024;

struct TankerStream<UserStream: AsyncRead + Unpin> {
    user_stream: Option<UserStream>,
    tanker_stream_handle: *mut tanker_stream_t,
    tanker_read_future: Option<CFuture<c_void>>,
    read_operation: Option<ReadOperation>,
    sender_bundle: SenderBundle,
    receiver: Receiver<ReadOperation>,
    buffer: Vec<u8>,
}

impl<UserStream: AsyncRead + Unpin> TankerStream<UserStream> {
    fn new() -> Self {
        let (sender, receiver) = bounded(1);
        TankerStream {
            user_stream: None,
            tanker_stream_handle: std::ptr::null_mut(),
            tanker_read_future: None,
            read_operation: None,
            sender_bundle: SenderBundle {
                sender,
                waker: Mutex::new(None),
            },
            receiver,
            buffer: Vec::with_capacity(STREAM_CHUNK_SIZE),
        }
    }
}

impl<UserStream: AsyncRead + Unpin> Drop for TankerStream<UserStream> {
    fn drop(&mut self) {
        // SAFETY: We first close the stream, which guarantees us that the
        // SenderBundle won't be used anymore. We never process any ReadOperation outside of
        // poll_read, which can't be called because the stream is being dropped. At the end of this
        // function, the channel will be closed and pending operations discarded.
        let fut = unsafe {
            CFuture::<c_void>::new(tanker_call_ext!(tanker_stream_close(
                self.tanker_stream_handle
            )))
        };
        if let Err(e) = block_on(fut) {
            if cfg!(debug_assertions) {
                panic!("Failed to close Tanker stream: {}", e);
            } else {
                eprintln!("Failed to close Tanker stream: {}", e);
            }
        }
    }
}

/// Perform ReadOperations that come through `receiver` on `user_stream`,
/// until `future` resolves.
/// This is needed for tanker_stream_decrypt because it will try to read the user stream
/// before it has returned a stream.
async fn process_stream_until<T, UserStream: AsyncRead + Unpin>(
    receiver: &mut Receiver<ReadOperation>,
    user_stream: &mut UserStream,
    mut future: impl Future<Output = Result<T, Error>> + Unpin,
) -> Result<T, futures::io::Error> {
    let fut_result = loop {
        let either = select(Box::pin(receiver.recv()), future).await;
        match either {
            Either::Left((read_operation, unused_future)) => {
                let read_operation = read_operation.unwrap();
                let buf = unsafe {
                    std::slice::from_raw_parts_mut(
                        read_operation.buffer,
                        read_operation.size as usize,
                    )
                };
                let read_size = user_stream.read(buf).await?;
                unsafe {
                    tanker_call_ext!(tanker_stream_read_operation_finish(
                        read_operation.operation,
                        read_size as i64,
                    ));
                }

                future = unused_future;
            }
            Either::Right((fut_result, _)) => {
                break fut_result?;
            }
        }
    };
    Ok(fut_result)
}

/// Callback given to the C API to read the channel.
unsafe extern "C" fn read_underlying_stream(
    buf: *mut u8,
    size: i64,
    op: *mut tanker_stream_read_operation_t,
    additional_data: *mut c_void,
) {
    debug_assert!(size >= 0);

    let sender_bundle =
        unsafe { (&mut *(additional_data as *mut SenderBundle)) as &mut SenderBundle };

    // Send the ReadOperation to the channel
    block_on(sender_bundle.sender.send(ReadOperation {
        buffer: buf,
        size,
        operation: op,
    }))
    .unwrap();

    // Wake the Waker.
    let mut waker = sender_bundle.waker.lock().unwrap();
    if let Some(waker) = waker.take() {
        waker.wake();
    }
}

impl<UserStream: AsyncRead + Unpin> AsyncRead for TankerStream<UserStream> {
    // Processes all pending operation until we manage to get
    // some bytes in self.buffer. Then, we drain those bytes every time
    // we are called, until the buffer is empty, then we ask to fill it again.
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        if self.buffer.is_empty() {
            // First, set the waker so that we don't miss anything.
            {
                let mut waker_guard = self.sender_bundle.waker.lock().unwrap();
                *waker_guard = Some(cx.waker().clone());
            }

            // Take the next ReadOperation if there is one
            match self.receiver.try_recv() {
                Ok(read_operation) => {
                    debug_assert!(
                        self.read_operation.is_none(),
                        "Tanker never asks for a ReadOperation if one is in progress"
                    );
                    self.read_operation = Some(read_operation);
                }
                Err(TryRecvError::Closed) => {
                    panic!("error reading channel: closed");
                }
                Err(TryRecvError::Empty) => {} // Channel still open, but no message
            }

            // Process the ReadOperation if there is one in progress
            if let Some(read_operation) = self.read_operation.as_mut() {
                let buf = unsafe {
                    std::slice::from_raw_parts_mut(
                        read_operation.buffer,
                        read_operation.size as usize,
                    )
                };
                let c_operation = read_operation.operation; // copy it now to satisfy the borrow checker
                let mut user_stream_pin = Pin::new(self.user_stream.as_mut().unwrap());
                let read_size = ready!(user_stream_pin.as_mut().poll_read(cx, buf))?;
                unsafe {
                    tanker_call_ext!(tanker_stream_read_operation_finish(
                        c_operation,
                        read_size as i64
                    ));
                }
                self.read_operation = None;
            }

            // Start a read on Tanker's stream if we haven't already
            if self.tanker_read_future.is_none() {
                unsafe {
                    self.tanker_read_future = Some(CFuture::<c_void>::new(tanker_call_ext!(
                        tanker_stream_read(
                            self.tanker_stream_handle,
                            self.buffer.as_mut_ptr(),
                            self.buffer.capacity() as i64,
                        )
                    )));
                }
            }

            // Wait for the read to finish
            let bytes_read = ready!(self.tanker_read_future.as_mut().unwrap().poll_unpin(cx))?;

            self.tanker_read_future = None;

            unsafe { self.buffer.set_len(bytes_read as usize) };
        }

        let to_read = min(self.buffer.len(), buf.len());
        buf[..to_read].copy_from_slice(self.buffer.drain(0..to_read).as_slice());

        Poll::Ready(Ok(to_read))
    }
}

pub async unsafe fn encrypt_stream<UserStream: AsyncRead + Unpin>(
    ctanker: CTankerPtr,
    user_stream: UserStream,
    options: &EncryptionOptions,
) -> Result<impl AsyncRead + Unpin, Error> {
    let options_wrapper = options.to_c_encryption_options();

    let mut tanker_stream = Box::new(TankerStream::new());
    tanker_stream.user_stream = Some(user_stream);

    let fut = unsafe {
        CFuture::<tanker_stream_t>::new(tanker_call_ext!(tanker_stream_encrypt(
            ctanker,
            Some(read_underlying_stream),
            (&mut tanker_stream.sender_bundle as *mut _) as *mut _,
            &options_wrapper.c_options,
        )))
    };
    tanker_stream.tanker_stream_handle = fut.await?;

    Ok(tanker_stream)
}

pub async unsafe fn encryption_session_encrypt_stream<UserStream: AsyncRead + Unpin>(
    csess: CEncSessPtr,
    user_stream: UserStream,
) -> Result<impl AsyncRead + Unpin, Error> {
    let mut tanker_stream = Box::new(TankerStream::new());
    tanker_stream.user_stream = Some(user_stream);

    let fut = unsafe {
        CFuture::<tanker_stream_t>::new(tanker_call_ext!(tanker_encryption_session_stream_encrypt(
            csess,
            Some(read_underlying_stream),
            (&mut tanker_stream.sender_bundle as *mut _) as *mut _,
        )))
    };
    tanker_stream.tanker_stream_handle = fut.await?;

    Ok(tanker_stream)
}

pub async unsafe fn decrypt_stream<UserStream: AsyncRead + Unpin>(
    ctanker: CTankerPtr,
    mut user_stream: UserStream,
) -> Result<impl AsyncRead + Unpin, Error> {
    let mut tanker_stream = Box::new(TankerStream::new());

    let fut = unsafe {
        CFuture::<tanker_stream_t>::new(tanker_call_ext!(tanker_stream_decrypt(
            ctanker,
            Some(read_underlying_stream),
            (&mut tanker_stream.sender_bundle as *mut _) as *mut _,
        )))
    };

    // Process all the ReadOperations until Tanker's stream is ready
    let stream_handle = process_stream_until(&mut tanker_stream.receiver, &mut user_stream, fut)
        .await
        .map_err(|e| {
            Error::new_with_source(
                ErrorCode::IoError,
                "IO error in Tanker stream".to_owned(),
                e,
            )
        })?;

    tanker_stream.tanker_stream_handle = stream_handle;
    tanker_stream.user_stream = Some(user_stream);

    Ok(tanker_stream)
}
