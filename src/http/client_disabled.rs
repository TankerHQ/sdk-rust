#[derive(Debug)]
pub struct HttpClient;

// Stub HttpClient used when the feature is disabled
impl HttpClient {
    pub async fn new(_sdk_type: Option<&str>) -> Self {
        Self
    }
}
