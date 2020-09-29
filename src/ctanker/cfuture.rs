use super::bindings::tanker_future;
use crate::Error;
use futures::channel::oneshot;
use std::ffi::{c_void, CStr};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

struct CFutureContext<T> {
    pub waker: Weak<Mutex<Option<Waker>>>,
    pub sender: oneshot::Sender<<CFuture<T> as Future>::Output>,
}

#[derive(Debug)]
pub struct CFuture<T> {
    cfut: *mut tanker_future,
    receiver: Option<oneshot::Receiver<<Self as Future>::Output>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T> CFuture<T> {
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

    unsafe fn get_result(cfut: *mut tanker_future) -> Option<*mut T> {
        unsafe {
            if super::tanker_future_is_ready(cfut) {
                Some(super::tanker_future_get_voidptr(cfut) as *mut T)
            } else {
                None
            }
        }
    }

    unsafe fn get_error(cfut: *mut tanker_future) -> Option<Error> {
        unsafe {
            if super::tanker_future_has_error(cfut) == 0 {
                return None;
            }

            let cerror = super::tanker_future_get_error(cfut);
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

impl<T> Drop for CFuture<T> {
    fn drop(&mut self) {
        unsafe { super::tanker_future_destroy(self.cfut) }
    }
}

impl<T> Future for CFuture<T> {
    type Output = Result<*mut T, Error>;

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
            let waker_fut =
                super::tanker_future_then(self.cfut, Some(Self::waker_callback), ctx_ptr);
            super::tanker_future_destroy(waker_fut);
        }

        Poll::Pending
    }
}
