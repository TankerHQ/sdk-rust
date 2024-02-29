mod rest;

pub use rest::admin_rest_request;

use super::App;
use super::OIDCProvider;
use crate::identity::test_app::OidcConfig;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::json;
use tankersdk::Error;

#[derive(Debug)]
pub struct Admin {
    app_management_url: String,
    client: reqwest::Client,
    environment_name: String,
    trustchain_url: String,
}

impl Admin {
    pub fn new(
        app_management_token: String,
        app_management_url: String,
        environment_name: String,
        trustchain_url: String,
    ) -> Result<Self, Error> {
        let headers = [
            (ACCEPT, "application/json"),
            (AUTHORIZATION, &format!("Bearer {app_management_token}")),
        ]
        .iter()
        .map(|(k, v)| (k.clone(), HeaderValue::from_str(v).unwrap()))
        .collect();
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        Ok(Self {
            app_management_url,
            client,
            environment_name,
            trustchain_url,
        })
    }

    pub async fn create_app(&self, name: &str) -> Result<App, Error> {
        let reply = admin_rest_request(self.client.post(&self.make_url("")).json(&json!({
            "name": name,
            "environment_name": &self.environment_name,
        })))
        .await?;

        let json_app = reply["app"].as_object().unwrap();

        Ok(App {
            url: self.trustchain_url.clone(),
            id: json_app["id"].as_str().unwrap().to_owned(),
            private_key: json_app["secret"].as_str().unwrap().to_owned(),
        })
    }

    pub async fn delete_app(&self, id: &str) -> Result<(), Error> {
        admin_rest_request(self.client.delete(&self.make_url(id))).await?;
        Ok(())
    }

    pub async fn app_update(
        &self,
        id: &str,
        oidc_provider: &OidcConfig,
    ) -> Result<OIDCProvider, Box<dyn std::error::Error>> {
        let url = self.make_url(id);
        let reply = admin_rest_request(self.client.patch(&url).json(&json!({
            "oidc_providers": [
                {
                    "issuer": oidc_provider.issuer.clone(),
                    "client_id": oidc_provider.client_id.clone(),
                    "display_name": oidc_provider.provider_name.clone(),
                }
            ]
        })))
        .await?;
        let invalid_response = "invalid response from the App Manangement API";

        let json_oidc_provider = reply["app"]["oidc_providers"][0]
            .as_object()
            .ok_or(invalid_response)?;
        Ok(OIDCProvider {
            id: json_oidc_provider["id"]
                .as_str()
                .ok_or(invalid_response)?
                .to_owned(),
            display_name: json_oidc_provider["display_name"]
                .as_str()
                .ok_or(invalid_response)?
                .to_owned(),
        })
    }

    fn make_url(&self, id: &str) -> String {
        let id = base64::decode(id).unwrap();
        let id = base64::encode_config(id, base64::URL_SAFE_NO_PAD);
        format!("{}/v2/apps/{}", &self.app_management_url, id)
    }
}
