use crate::ctanker::*;

pub type CHttpRequestHandle = *mut tanker_http_request_handle_t;
pub type CHttpHeader = tanker_http_header;

#[derive(Debug)]
pub struct CHttpRequest(pub(crate) *mut tanker_http_request_t);

// SAFETY: ctanker is thread-safe
unsafe impl Send for CHttpRequest {}

pub unsafe extern "C" fn send_http_request(
    creq_ptr: *mut tanker_http_request,
    data: *mut c_void,
) -> CHttpRequestHandle {
    let client = data as *const HttpClient;

    // client is an Arc on the Rust side, but it's a raw pointer held by native, so Rust can't
    // fully track its lifetime. Since Arc::drop will decrement the count, we must increment it
    unsafe { Arc::increment_strong_count(client) };

    // SAFETY: data is set to an Arc<HttpClient> in the tanker_options struct,
    // and we trust native to not send requests after Core has been dropped
    let client = unsafe { Arc::from_raw(client) };

    // SAFETY: We trust the request struct from native
    let req = unsafe { crate::http::request::HttpRequest::new(CHttpRequest(creq_ptr)) };
    let req_handle = client.send_request(req);

    // NOTE: If/when strict provenance is stabilized, this should be a std::ptr::invalid()
    req_handle as CHttpRequestHandle
}

pub unsafe extern "C" fn cancel_http_request(
    creq_ptr: *mut tanker_http_request,
    handle: CHttpRequestHandle,
    data: *mut c_void,
) {
    let client = data as *const HttpClient;

    // client is an Arc on the Rust side, but it's a raw pointer held by native, so Rust can't
    // fully track its lifetime. Since Arc::drop will decrement the count, we must increment it
    unsafe { Arc::increment_strong_count(client) };

    // SAFETY: data is set to an Arc<HttpClient> in the tanker_options struct,
    // and we trust native to not send requests after Core has been dropped
    let client = unsafe { Arc::from_raw(client) };

    // SAFETY: We trust the request struct from native
    let req = unsafe { crate::http::request::HttpRequest::new(CHttpRequest(creq_ptr)) };
    client.cancel_request(req, handle as usize);
}

impl CTankerLib {
    pub unsafe fn http_handle_response(
        &self,
        request: CHttpRequest,
        response: crate::http::response::HttpResponse,
    ) {
        let cheaders = response
            .headers
            .iter()
            .map(|header| tanker_http_header_t {
                name: header.name.as_ptr() as *const c_char,
                value: header.value.as_ptr() as *const c_char,
            })
            .collect::<Vec<_>>();

        let mut cresponse = tanker_http_response_t {
            error_msg: response
                .error_msg
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(std::ptr::null()),
            headers: cheaders.as_ptr() as *mut tanker_http_header_t,
            num_headers: response.headers.len() as i32,
            body: response
                .body
                .as_ref()
                .map(|s| s.as_ptr() as *const c_char)
                .unwrap_or(std::ptr::null()),
            body_size: response.body.as_ref().map(|v| v.len()).unwrap_or(0) as i64,
            status_code: response.status_code as i32,
        };

        unsafe { tanker_call!(self, tanker_http_handle_response(request.0, &mut cresponse)) };
    }
}
