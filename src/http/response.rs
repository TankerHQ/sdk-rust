use bytes::Bytes;
use std::ffi::CString;

#[derive(Debug)]
pub struct HttpResponse {
    pub(crate) error_msg: Option<CString>,
    pub(crate) content_type: Option<CString>,
    pub(crate) body: Option<Bytes>,
    pub(crate) status_code: u32,
}

impl HttpResponse {
    pub fn new(status_code: u32, content_type: Option<&str>, body: Bytes) -> Self {
        Self {
            error_msg: None,
            status_code,
            content_type: content_type.map(|s| CString::new(s).unwrap()),
            body: Some(body),
        }
    }

    pub fn new_network_error(error_msg: &str) -> Self {
        Self {
            error_msg: Some(CString::new(error_msg).unwrap()),
            status_code: 0,
            content_type: None,
            body: None,
        }
    }

    pub fn new_body_error(status_code: u32, content_type: Option<&str>, error_msg: &str) -> Self {
        Self {
            error_msg: Some(CString::new(error_msg).unwrap()),
            status_code,
            content_type: content_type.map(|s| CString::new(s).unwrap()),
            body: None,
        }
    }
}
