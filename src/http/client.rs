use super::response::{HttpResponse, HttpResponseHeader};
use crate::ctanker::{CTankerLib, RUST_SDK_TYPE, RUST_SDK_VERSION};
use crate::http::{request::HttpRequest, HttpRequestId};
use reqwest::{redirect::Policy, Client, Method, Request, Response};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct HttpClient {
    client: Client,
    sdk_type: String,
    next_id: AtomicUsize,
    _runtime: Option<tokio::runtime::Runtime>,
    handle: tokio::runtime::Handle,
    // NOTE: This is a *sync* mutex, don't lock this in async code
    req_handles: Mutex<HashMap<HttpRequestId, JoinHandle<()>>>,
}

impl HttpClient {
    pub async fn new(sdk_type: Option<&str>) -> Self {
        let (handle, runtime) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (h, None),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("tanker-http-tokio")
                    .build()
                    .unwrap();
                (rt.handle().clone(), Some(rt))
            }
        };

        Self {
            client: Client::builder().redirect(Policy::none()).build().unwrap(),
            sdk_type: sdk_type.unwrap_or(RUST_SDK_TYPE).to_string(),
            next_id: 0.into(),
            _runtime: runtime,
            handle,
            req_handles: Mutex::new(Default::default()),
        }
    }

    pub fn send_request(self: Arc<Self>, native_req: HttpRequest) -> HttpRequestId {
        let req_id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let handle = self.handle.clone();
        let req_handle = handle.spawn(self.clone().do_request_async(req_id, native_req));
        self.req_handles.lock().unwrap().insert(req_id, req_handle);
        req_id
    }

    fn build_request(self: Arc<Self>, native_req: &HttpRequest) -> Result<Request, HttpResponse> {
        let method = match Method::from_str(native_req.method) {
            Ok(m) => m,
            Err(e) => return Err(HttpResponse::new_network_error(&e.to_string())),
        };
        let mut req_builder = self
            .client
            .request(method, native_req.url)
            .body(native_req.body);

        for header in &native_req.headers {
            req_builder = req_builder.header(header.name, header.value);
        }

        let mut req = match req_builder.build() {
            Ok(r) => r,
            Err(e) => return Err(HttpResponse::new_misc_error(&e.to_string())),
        };
        let headers = req.headers_mut();
        headers.insert("X-Tanker-SdkType", self.sdk_type.parse().unwrap());
        headers.insert("X-Tanker-SdkVersion", RUST_SDK_VERSION.parse().unwrap());
        if native_req.body.is_empty() {
            headers.insert("Content-Length", "0".parse().unwrap());
        }
        Ok(req)
    }

    async fn do_request_async(self: Arc<Self>, req_id: HttpRequestId, native_req: HttpRequest) {
        let response = match self.clone().build_request(&native_req) {
            Ok(req) => match self.client.execute(req).await {
                Ok(r) => Self::read_response(r).await,
                Err(e) => HttpResponse::new_network_error(&e.to_string()),
            },
            Err(e) => e,
        };

        self.clone()
            .handle
            .spawn_blocking(move || self.request_complete(req_id));

        // SAFETY: All raw pointer come from native, so they are trusted
        unsafe { CTankerLib::get().http_handle_response(native_req.crequest, response) }
    }

    async fn read_response(response: Response) -> HttpResponse {
        let status = response.status().as_u16() as u32;
        let headers = response
            .headers()
            .into_iter()
            .map(|(k, v)| HttpResponseHeader::new(k.to_string(), v.as_bytes().to_vec()))
            .collect::<Result<Vec<_>, String>>();

        let headers = match headers {
            Ok(e) => e,
            Err(header_name) => {
                return HttpResponse::new_misc_error(&format!(
                    "header '{header_name}' contains null bytes"
                ))
            }
        };

        match response.bytes().await {
            Ok(data) => HttpResponse::new(status, headers, data),
            Err(e) => HttpResponse::new_body_error(status, headers, &e.to_string()),
        }
    }

    // NOTE: This locks a *sync* mutex, do not call directly from async code
    fn request_complete(&self, req_id: HttpRequestId) {
        self.req_handles.lock().unwrap().remove(&req_id);
    }

    pub fn cancel_request(&self, _req: HttpRequest, req_id: HttpRequestId) {
        if let Some(handle) = self.req_handles.lock().unwrap().remove(&req_id) {
            handle.abort();
        }
    }
}
