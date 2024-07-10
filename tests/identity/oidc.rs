#![allow(dead_code)] // This module is compiled per-test. Not all tests will use all functions!

use crate::identity::test_app::OidcConfig;
use base64::prelude::*;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct OIDCProvider {
    pub id: String,
    pub display_name: String,
}

pub async fn get_id_token(oidc: &OidcConfig) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://www.googleapis.com/oauth2/v4/token")
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": &oidc.users["martine"].refresh_token,
            "client_id": &oidc.client_id,
            "client_secret": &oidc.client_secret,
        }))
        .send()
        .await?;
    let reply: Value = response.json().await?;
    let oidc_token = reply["id_token"]
        .as_str()
        .ok_or("invalid response from google oidc")?
        .to_owned();
    Ok(oidc_token)
}

pub fn extract_subject(id_token: &str) -> Result<String, Box<dyn std::error::Error>> {
    let jwt_body = id_token.split('.').nth(1).ok_or("invalid ID Token")?;
    let body = BASE64_URL_SAFE_NO_PAD.decode(jwt_body)?;
    let json_body: Value = serde_json::from_slice(&body[..])?;
    Ok(json_body["sub"]
        .as_str()
        .ok_or("invalid ID Token")?
        .to_owned())
}
