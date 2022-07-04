use crate::ctanker::{CTankerLib, RUST_SDK_TYPE, RUST_SDK_VERSION};
use crate::http::{HttpRequest, HttpRequestId, HttpResponse};
use reqwest::{Client, Method, Response};
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
            client: Client::new(),
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

    async fn do_request_async(self: Arc<Self>, req_id: HttpRequestId, native_req: HttpRequest) {
        let method = match Method::from_str(native_req.method) {
            Ok(m) => m,
            Err(e) => {
                self.clone()
                    .handle
                    .spawn_blocking(move || self.request_complete(req_id));

                // SAFETY: crequest comes from native (lives until handle_response returns)
                let resp = HttpResponse::new_network_error(&e.to_string());
                unsafe { CTankerLib::get().http_handle_response(native_req.crequest, resp) };
                return;
            }
        };
        let mut req_builder = self
            .client
            .request(method, native_req.url)
            .header("X-Tanker-SdkType", &self.sdk_type)
            .header("X-Tanker-SdkVersion", RUST_SDK_VERSION)
            .header("X-Tanker-Instanceid", native_req.instance_id)
            .body(native_req.body);
        if let Some(auth) = native_req.authorization {
            req_builder = req_builder.header("Authorization", auth);
        }

        let response = match req_builder.send().await {
            Ok(r) => Self::read_response(r).await,
            Err(e) => HttpResponse::new_network_error(&e.to_string()),
        };

        self.clone()
            .handle
            .spawn_blocking(move || self.request_complete(req_id));

        // SAFETY: All raw pointer come from native, so they are trusted
        unsafe { CTankerLib::get().http_handle_response(native_req.crequest, response) }
    }

    async fn read_response(response: Response) -> HttpResponse {
        let status = response.status().as_u16() as u32;
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().map(ToString::to_string).ok());
        match response.bytes().await {
            Ok(data) => HttpResponse::new(status, content_type.as_deref(), data),
            Err(e) => HttpResponse::new_body_error(status, content_type.as_deref(), &e.to_string()),
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
