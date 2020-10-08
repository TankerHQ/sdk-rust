mod block;
use block::serialized_root_block;

use super::App;
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use serde_json::Value;
use tankersdk::{Error, ErrorCode};

#[derive(Debug)]
pub struct Admin {
    client: reqwest::Client,
    admin_url: String,
    api_url: String,
    id_token: String,
}

impl Admin {
    pub async fn new(admin_url: String, id_token: String, api_url: String) -> Result<Self, Error> {
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
            api_url,
            id_token,
        })
    }

    pub async fn create_app(&self, name: &str, is_test: bool) -> Result<App, Error> {
        let sign_keypair = Keypair::generate(&mut OsRng {});
        let private_key_b64 = base64::encode(sign_keypair.to_bytes().as_ref());

        let root_block = serialized_root_block(&sign_keypair);
        let serialized_block = base64::encode(&root_block);

        let mut json = [("name", name), ("root_block", &serialized_block)]
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

        let reply = Self::do_request(self.client.post(&self.make_url("")).json(&json)).await?;
        let reply: Value = serde_json::from_str(&reply).unwrap();

        let json_app = &reply.as_object().unwrap()["app"];
        let json_app = json_app.as_object().unwrap();

        Ok(App {
            url: self.api_url.clone(),
            id: json_app["id"].as_str().unwrap().to_owned(),
            auth_token: json_app["auth_token"].as_str().unwrap().to_owned(),
            private_key: private_key_b64,
        })
    }

    pub async fn delete_app(&self, id: &str) -> Result<(), Error> {
        Self::do_request(self.client.delete(&self.make_url(id))).await?;
        Ok(())
    }

    pub async fn app_update(
        &self,
        id: &str,
        oidc_client_id: Option<&str>,
        oidc_provider: Option<&str>,
    ) -> Result<(), Error> {
        let url = self.make_url(id);
        let mut json = serde_json::Map::<_, _>::new();
        if let Some(oidc_client_id) = oidc_client_id {
            json.insert("oidc_client_id".to_owned(), oidc_client_id.into());
        }
        if let Some(oidc_provider) = oidc_provider {
            json.insert("oidc_provider".to_owned(), oidc_provider.into());
        }
        let json: Value = json.into();

        Self::do_request(self.client.patch(&url).json(&json)).await?;
        Ok(())
    }

    async fn do_request(req: reqwest::RequestBuilder) -> Result<String, Error> {
        let reply = match req.send().await {
            Err(e) => {
                return Err(Error::new_with_source(
                    ErrorCode::NetworkError,
                    "app_update network request failed".into(),
                    e,
                ))
            }
            Ok(reply) => reply,
        };

        let status = reply.status();
        let reply = reply.text().await.unwrap();

        if status.is_success() {
            Ok(reply)
        } else {
            Err(Error::new(
                ErrorCode::InternalError,
                format!("Failed to update app: {}", reply),
            ))
        }
    }

    fn make_url(&self, id: &str) -> String {
        let id = base64::decode(id).unwrap();
        let id = base64::encode_config(id, base64::URL_SAFE_NO_PAD);
        format!("{}/apps/{}", &self.admin_url, id)
    }
}
