use super::admin::admin_rest_request;
use reqwest::header::ACCEPT;
use serde_json::json;
use tankersdk::Error;

#[derive(Debug, Clone)]
pub struct App {
    pub url: String,
    pub id: String,
    pub auth_token: String,
    pub private_key: String,
}

impl App {
    pub async fn get_verification_code(&self, email: &str) -> Result<String, Error> {
        let client = reqwest::Client::new();
        let reply = admin_rest_request(
            client
                .post(&format!("{}/verification/email/code", &self.url))
                .json(
                    &json!({ "email": email, "app_id": &self.id, "auth_token": &self.auth_token }),
                )
                .header(ACCEPT, "application/json"),
        )
        .await?;

        let code = reply["verification_code"].as_str().unwrap().to_owned();
        Ok(code)
    }
}
