use std::ffi::CString;
use tankersdk::{cadmin, Error};

#[derive(Debug, Clone)]
pub struct App {
    pub url: String,
    pub id: String,
    pub auth_token: String,
    pub private_key: String,
}

impl App {
    pub async fn get_verification_code(&self, email: &str) -> Result<String, Error> {
        let curl = CString::new(self.url.as_str()).unwrap();
        let cid = CString::new(self.id.as_str()).unwrap();
        let cauth_token = CString::new(self.auth_token.as_str()).unwrap();
        let cemail = CString::new(email).unwrap();
        cadmin::get_verification_code(&curl, &cid, &cauth_token, &cemail).await
    }
}
