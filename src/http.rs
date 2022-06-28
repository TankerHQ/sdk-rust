#[cfg_attr(not(feature = "http"), path = "http/client_disabled.rs")]
mod client;
pub use client::HttpClient;

#[cfg(feature = "http")]
mod response;
#[cfg(feature = "http")]
pub use response::HttpResponse;

use crate::ctanker::CHttpRequest;

#[cfg(feature = "http")]
pub type HttpRequestId = usize;

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
