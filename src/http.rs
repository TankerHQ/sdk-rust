#[cfg_attr(not(feature = "http"), path = "http/client_disabled.rs")]
mod client;
pub use client::HttpClient;

#[cfg(feature = "http")]
mod response;
#[cfg(feature = "http")]
pub use response::HttpResponse;

#[cfg(feature = "http")]
pub type HttpRequestId = usize;

use crate::ctanker::CHttpRequest;

// NOTE: The member lifetimes are marked as 'static, but they really have the lifetime of crequest,
//       which cannot easily be expressed (lifetime is essentially 'self, without self-references).
#[derive(Debug)]
#[cfg_attr(not(feature = "http"), allow(dead_code))]
pub struct HttpRequest {
    pub(crate) crequest: CHttpRequest,
    pub(crate) method: &'static str,
    pub(crate) url: &'static str,
    pub(crate) instance_id: &'static str,
    pub(crate) authorization: Option<&'static str>,
    pub(crate) body: &'static [u8],
}

#[cfg(feature = "http")]
impl HttpRequest {
    pub unsafe fn new(crequest: CHttpRequest) -> Self {
        use std::ffi::CStr;

        // SAFETY: We trust that native strings are UTF-8 and the pointer/sizes are valid
        let creq = unsafe { &*crequest.0 };
        let authorization = if (*creq).authorization.is_null() {
            None
        } else {
            Some(CStr::from_ptr(creq.authorization).to_str().unwrap())
        };
        Self {
            crequest,
            method: CStr::from_ptr(creq.method).to_str().unwrap(),
            url: CStr::from_ptr(creq.url).to_str().unwrap(),
            instance_id: CStr::from_ptr(creq.instance_id).to_str().unwrap(),
            authorization,
            body: std::slice::from_raw_parts(creq.body as *const u8, creq.body_size as usize),
        }
    }
}
