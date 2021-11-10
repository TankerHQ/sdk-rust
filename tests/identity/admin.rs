mod rest;

pub use rest::admin_rest_request;

mod block;

use block::serialized_root_block;

use super::App;
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::Value;
use tankersdk::Error;

#[derive(Debug)]
pub struct Admin {
    client: reqwest::Client,
    admin_url: String,
    trustchain_url: String,
    api_url: String,
    id_token: String,
}

impl Admin {
    pub fn new(
        admin_url: String,
        id_token: String,
        api_url: String,
        trustchain_url: String,
    ) -> Result<Self, Error> {
        let headers = [
            (ACCEPT, "application/json"),
            (AUTHORIZATION, &format!("Bearer {}", id_token)),
        ]
        .iter()
        .map(|(k, v)| (k.clone(), HeaderValue::from_str(v).unwrap()))
        .collect();
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        Ok(Self {
            client,
            admin_url,
            trustchain_url,
            api_url,
            id_token,
        })
    }

    pub async fn get_environments(&self) -> Result<Vec<String>, Error> {
        let reply =
            admin_rest_request(self.client.get(format!("{}/environments", self.admin_url))).await?;
        let environments_ids = reply["environments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();

        Ok(environments_ids)
    }

    pub async fn create_app(&self, name: &str, is_test: bool) -> Result<App, Error> {
        let envs = self.get_environments().await?;
        assert!(!envs.is_empty(), "found 0 environments");

        let sign_keypair = Keypair::generate(&mut OsRng {});
        let private_key_b64 = base64::encode(sign_keypair.to_bytes().as_ref());

        let root_block = serialized_root_block(&sign_keypair);
        let serialized_block = base64::encode(&root_block);

        let mut json = [
            ("name", name),
            ("root_block", &serialized_block),
            ("environment_id", &envs[0]),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect::<serde_json::Map<_, _>>();
        if is_test {
            json.insert(
                "private_signature_key".to_owned(),
                Value::String(private_key_b64.clone()),
            );
        }
        let json: Value = json.into();

        let reply = admin_rest_request(self.client.post(&self.make_url("")).json(&json)).await?;
        let json_app = reply["app"].as_object().unwrap();

        Ok(App {
            url: self.trustchain_url.clone(),
            id: json_app["id"].as_str().unwrap().to_owned(),
            auth_token: json_app["auth_token"].as_str().unwrap().to_owned(),
            private_key: private_key_b64,
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
        with_session_token: Option<bool>,
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
        if let Some(with_session_token) = with_session_token {
            json.insert(
                "session_certificates_enabled".to_owned(),
                with_session_token.into(),
            );
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
        format!("{}/apps/{}", &self.admin_url, id)
    }
}
