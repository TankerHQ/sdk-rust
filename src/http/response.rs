use bytes::Bytes;
use std::ffi::CString;

#[derive(Debug)]
pub struct HttpResponseHeader {
    pub(crate) name: CString,
    pub(crate) value: CString,
}

impl HttpResponseHeader {
    pub fn new(name: String, value: Vec<u8>) -> Result<Self, String> {
        let Ok(value) = CString::new(value) else {
            return Err(name);
        };
        Ok(Self {
            name: CString::new(name).unwrap(),
            value,
        })
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    pub(crate) error_msg: Option<CString>,
    pub(crate) headers: Vec<HttpResponseHeader>,
    pub(crate) body: Option<Bytes>,
    pub(crate) status_code: u32,
}

impl HttpResponse {
    pub fn new(status_code: u32, headers: Vec<HttpResponseHeader>, body: Bytes) -> Self {
        Self {
            error_msg: None,
            status_code,
            headers,
            body: Some(body),
        }
    }

    pub fn new_network_error(error_msg: &str) -> Self {
        Self {
            error_msg: Some(CString::new(error_msg).unwrap()),
            status_code: 0,
            headers: vec![],
            body: None,
        }
    }

    pub fn new_misc_error(error_msg: &str) -> Self {
        Self {
            error_msg: Some(CString::new(error_msg).unwrap()),
            status_code: 0,
            headers: vec![],
            body: None,
        }
    }

    pub fn new_body_error(
        status_code: u32,
        headers: Vec<HttpResponseHeader>,
        error_msg: &str,
    ) -> Self {
        Self {
            error_msg: Some(CString::new(error_msg).unwrap()),
            status_code,
            headers,
            body: None,
        }
    }
}
