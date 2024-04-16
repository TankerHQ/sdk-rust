use crate::ctanker::{CHttpHeader, CHttpRequest};
use crate::Error;

// NOTE: The member lifetimes are marked as 'static, but they really have the lifetime of crequest,
//       which cannot easily be expressed (lifetime is essentially 'self, without self-references).
#[derive(Debug)]
pub struct HttpRequestHeader {
    pub(crate) name: &'static str,
    pub(crate) value: &'static str,
}

impl HttpRequestHeader {
    pub(crate) fn try_from(header: &CHttpHeader) -> Result<Self, Error> {
        use std::ffi::CStr;

        // SAFETY: We trust that native strings are UTF-8 and the pointer/sizes are valid
        let c_name = unsafe { CStr::from_ptr(header.name) };
        let name = c_name.to_str().unwrap();

        // SAFETY: We trust that native strings are UTF-8 and the pointer/sizes are valid
        let c_value = unsafe { CStr::from_ptr(header.value) };
        let value = c_value.to_str().unwrap();

        Ok(Self { name, value })
    }
}

// NOTE: The member lifetimes are marked as 'static, but they really have the lifetime of crequest,
//       which cannot easily be expressed (lifetime is essentially 'self, without self-references).
#[derive(Debug)]
pub struct HttpRequest {
    pub(crate) crequest: CHttpRequest,
    pub(crate) method: &'static str,
    pub(crate) url: &'static str,
    pub(crate) headers: Vec<HttpRequestHeader>,
    pub(crate) body: &'static [u8],
}

impl HttpRequest {
    pub unsafe fn new(crequest: CHttpRequest) -> Self {
        use std::ffi::CStr;

        // SAFETY: We trust that native strings are UTF-8 and the pointer/sizes are valid
        let creq = unsafe { &*crequest.0 };

        let headers = std::slice::from_raw_parts(creq.headers, creq.num_headers as usize)
            .iter()
            .map(|cheader: &CHttpHeader| HttpRequestHeader::try_from(cheader).unwrap())
            .collect();

        Self {
            crequest,
            method: CStr::from_ptr(creq.method).to_str().unwrap(),
            url: CStr::from_ptr(creq.url).to_str().unwrap(),
            headers,
            body: std::slice::from_raw_parts(creq.body as *const u8, creq.body_size as usize),
        }
    }
}
