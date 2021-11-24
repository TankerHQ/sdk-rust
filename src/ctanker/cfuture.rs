use super::bindings::tanker_future;
use crate::ctanker::*;
use crate::Error;
use futures::channel::oneshot;
use std::ffi::{c_void, CStr};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

/// # Safety
/// We know the c_void argument comes from ctanker, so all the unsafe impls can safely assume this
pub(crate) unsafe trait FromRawFuturePointer {
    fn from(ptr: *mut c_void) -> Self;
}

unsafe impl FromRawFuturePointer for () {
    fn from(_ptr: *mut c_void) -> Self {}
}

unsafe impl<T> FromRawFuturePointer for *mut T {
    fn from(ptr: *mut c_void) -> Self {
        ptr as *mut T
    }
}

unsafe impl FromRawFuturePointer for CTankerPtr {
    fn from(ptr: *mut c_void) -> Self {
        CTankerPtr(ptr as *mut tanker)
    }
}

unsafe impl FromRawFuturePointer for CStreamPtr {
    fn from(ptr: *mut c_void) -> Self {
        CStreamPtr(ptr as *mut tanker_stream)
    }
}

unsafe impl FromRawFuturePointer for u32 {
    fn from(ptr: *mut c_void) -> Self {
        ptr as u32
    }
}

unsafe impl FromRawFuturePointer for usize {
    fn from(ptr: *mut c_void) -> Self {
        ptr as usize
    }
}

unsafe impl FromRawFuturePointer for String {
    fn from(ptr: *mut c_void) -> Self {
        // SAFETY: CFuture::new is unsafe, so caller ensures the pointer is valid
        unsafe {
            let str = CStr::from_ptr(ptr as *const c_char)
                .to_str()
                .unwrap()
                .to_owned();
            CTankerLib::get().free_buffer(ptr);
            str
        }
    }
}

unsafe impl FromRawFuturePointer for Option<String> {
    fn from(ptr: *mut c_void) -> Self {
        let str_ptr = ptr as *mut c_char;
        NonNull::new(str_ptr).map(|str_ptr| {
            // SAFETY: CFuture::new is unsafe, so caller ensures the pointer is valid
            let str = unsafe { CStr::from_ptr(str_ptr.as_ptr()) }
                .to_str()
                .unwrap()
                .to_owned();
            unsafe { CTankerLib::get().free_buffer(ptr) };
            str
        })
    }
}

struct CFutureContext<T: FromRawFuturePointer> {
    pub waker: Weak<Mutex<Option<Waker>>>,
    pub sender: oneshot::Sender<<CFuture<T> as Future>::Output>,
}

#[derive(Debug)]
pub(crate) struct CFuture<T: FromRawFuturePointer> {
    cfut: *mut tanker_future,
    receiver: Option<oneshot::Receiver<<Self as Future>::Output>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

// SAFETY: ctanker is thread-safe
// NOTE: We can promise CFuture<T> is Send only if T is Send (because of the oneshot channel)
unsafe impl<T: Send + FromRawFuturePointer> Send for CFuture<T> where
    <CFuture<T> as Future>::Output: Send
{
}

impl<T: FromRawFuturePointer> CFuture<T> {
    // SAFETY: Because Self::new is unsafe, other functions can assume self.cfut is valid,
    //          HOWEVER, it is unsound to call any tanker_future_* function except _destroy
    //          after tanker_future_then has been called, so the associated funcs are still unsafe!
    pub unsafe fn new(cfut: *mut tanker_future) -> Self {
        Self {
            cfut,
            receiver: None,
            waker: Mutex::new(None).into(),
        }
    }

    unsafe fn get_result(cfut: *mut tanker_future) -> Option<T> {
        unsafe {
            if tanker_call_ext!(tanker_future_is_ready(cfut)) {
                Some(T::from(tanker_call_ext!(tanker_future_get_voidptr(cfut))))
            } else {
                None
            }
        }
    }

    unsafe fn get_error(cfut: *mut tanker_future) -> Option<Error> {
        unsafe {
            if tanker_call_ext!(tanker_future_has_error(cfut)) == 0 {
                return None;
            }

            let cerror = tanker_call_ext!(tanker_future_get_error(cfut));
            let code = (*cerror).code.into();
            let msg = CStr::from_ptr((*cerror).message);
            let msg = msg.to_str().expect("Tanker errors are UTF-8").to_string();

            Some(Error::new(code, msg))
        }
    }

    unsafe extern "C" fn waker_callback(cfut: *mut tanker_future, arg: *mut c_void) -> *mut c_void {
        // SAFETY: This ptr comes from a Box::into_raw in Self::poll through tanker_future_then
        let ctx_ptr = arg as *mut CFutureContext<T>;
        let ctx = unsafe { Box::from_raw(ctx_ptr) };

        // SAFETY: This cfut is brand new, OK to call tanker_future_* functions on it
        unsafe {
            // If the other side was dropped, that's fine, we can just ignore the sender error
            if let Some(err) = Self::get_error(cfut) {
                let _ = ctx.sender.send(Err(err));
            } else if let Some(result) = Self::get_result(cfut) {
                let _ = ctx.sender.send(Ok(result));
            } else {
                unreachable!("waker_callback must be called with a ready cfuture")
            }
        }

        // Keep after the send(), to prevent a race with poll()
        if let Some(waker_arc) = ctx.waker.upgrade() {
            if let Ok(mut waker_guard) = waker_arc.lock() {
                if let Some(waker) = waker_guard.take() {
                    waker.wake();
                }
            }
        }

        std::ptr::null_mut()
    }
}

impl<T: FromRawFuturePointer> Drop for CFuture<T> {
    fn drop(&mut self) {
        unsafe { tanker_call_ext!(tanker_future_destroy(self.cfut)) }
    }
}

impl<T: FromRawFuturePointer> Future for CFuture<T> {
    type Output = Result<T, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If poll returns Pending, it promises to wake the Waker that it just received
        // We register just one tanker_future_then callback, but always using the last Pending Waker

        // 1. First thing we do is update our waker, so any future wake() will use the new waker
        let waker = cx.waker().clone();
        {
            let mut guard = self.waker.lock().unwrap();
            *guard = Some(waker);
        }

        // 2. If nothing was received, we know wake() is yet to be called (per waker_callback())
        // Therefore, wake() hasn't been called yet, and it will use our waker, so Pending is OK
        if let Some(receiver) = self.receiver.as_mut() {
            return if let Some(result) = receiver.try_recv().unwrap() {
                Poll::Ready(result)
            } else {
                Poll::Pending
            };
        }

        // SAFETY: OK because this is only called once, before doing tanker_future_then
        // This is the fast-path for futures that are created ready (tanker_expected_t)
        unsafe {
            if let Some(err) = Self::get_error(self.cfut) {
                return Poll::Ready(Err(err));
            } else if let Some(result) = Self::get_result(self.cfut) {
                return Poll::Ready(Ok(result));
            }
        }

        let (sender, receiver) = oneshot::channel();
        self.receiver = Some(receiver);

        let context = CFutureContext::<T> {
            waker: Arc::downgrade(&self.waker),
            sender,
        };
        let ctx_ptr = Box::into_raw(Box::new(context)) as *mut c_void;

        // SAFETY: Called once on a fresh cfut because we set self.receiver above
        unsafe {
            let waker_fut = tanker_call_ext!(tanker_future_then(
                self.cfut,
                Some(Self::waker_callback),
                ctx_ptr
            ));
            tanker_call_ext!(tanker_future_destroy(waker_fut));
        }

        Poll::Pending
    }
}
