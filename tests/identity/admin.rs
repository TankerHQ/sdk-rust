mod rest;

pub use rest::admin_rest_request;

use super::App;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::json;
use serde_json::Value;
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
            (AUTHORIZATION, &format!("Bearer {}", app_management_token)),
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
            auth_token: json_app["auth_token"].as_str().unwrap().to_owned(),
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
        oidc_client_id: Option<&str>,
        oidc_provider: Option<&str>,
        preverified_verification: Option<bool>,
    ) -> Result<(), Error> {
        let url = self.make_url(id);
        let mut json = serde_json::Map::<_, _>::new();
        if let Some(oidc_client_id) = oidc_client_id {
            json.insert("oidc_client_id".to_owned(), oidc_client_id.into());
        }
        if let Some(oidc_provider) = oidc_provider {
            json.insert("oidc_provider".to_owned(), oidc_provider.into());
        }
        if let Some(preverified_verification) = preverified_verification {
            json.insert(
                "preverified_verification_enabled".to_owned(),
                preverified_verification.into(),
            );
        }
        let json: Value = json.into();

        admin_rest_request(self.client.patch(&url).json(&json)).await?;
        Ok(())
    }

    fn make_url(&self, id: &str) -> String {
        let id = base64::decode(id).unwrap();
        let id = base64::encode_config(id, base64::URL_SAFE_NO_PAD);
        format!("{}/v1/apps/{}", &self.app_management_url, id)
    }
}
