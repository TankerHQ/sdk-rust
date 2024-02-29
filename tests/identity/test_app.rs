#![allow(dead_code)] // This module is compiled per-test. Not all tests will use all functions!

mod config;

use super::Admin;
use super::App;
use super::OIDCProvider;
use crate::identity::{create_identity, create_provisional_identity, get_public_identity};
pub use config::{Config, OidcConfig};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::future::Future;
use tankersdk::{Core, Error, LogRecordLevel, Options, Status, Verification, VerificationOptions};

pub struct TestApp {
    config: Config,
    admin: Admin,
    app: App,
}

impl TestApp {
    pub async fn get() -> Self {
        Core::set_log_handler(Box::new(|record| {
            if record.level >= LogRecordLevel::Warning {
                println!(
                    "[{} {}#{}:{}] {}",
                    record.level, record.category, record.file, record.line, record.message
                )
            }
        }));

        TestApp::new().await
    }

    pub fn make_options(&self) -> Options {
        Options::new(
            self.id().to_owned(),
            ":memory:".to_owned(),
            ":memory:".to_string(),
        )
        .with_url(self.config.api_url.clone())
        .with_sdk_type("sdk-rust-test".to_string())
    }

    async fn new() -> Self {
        let config = Config::new();
        let admin = Admin::new(
            config.app_management_token.clone(),
            config.app_management_url.clone(),
            config.environment_name.clone(),
            config.trustchain_url.clone(),
        )
        .unwrap();
        let app = admin.create_app("sdk-rust-tests").await.unwrap();
        Self { config, admin, app }
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub fn url(&self) -> &str {
        &self.config.api_url
    }

    pub fn trustchaind_url(&self) -> &str {
        &self.config.trustchain_url
    }

    pub fn verification_api_token(&self) -> &str {
        &self.config.verification_api_token
    }

    pub async fn get_email_verification_code(&self, email: &str) -> Result<String, Error> {
        self.app
            .get_email_verification_code(&self.config.verification_api_token, email)
            .await
    }

    pub async fn get_sms_verification_code(&self, phone_number: &str) -> Result<String, Error> {
        self.app
            .get_sms_verification_code(&self.config.verification_api_token, phone_number)
            .await
    }

    pub async fn app_update(
        &self,
        oidc_provider: &OidcConfig,
    ) -> Result<OIDCProvider, Box<dyn std::error::Error>> {
        self.admin.app_update(&self.app.id, oidc_provider).await
    }

    pub fn create_identity(&self, email: Option<String>) -> String {
        let user_id = email.unwrap_or_else(|| {
            rand::thread_rng()
                .sample_iter(Alphanumeric)
                .map(char::from)
                .take(30)
                .collect()
        });

        create_identity(&self.app.id, &self.app.private_key, &user_id).unwrap()
    }

    pub fn create_provisional_identity(&self, email: &str) -> String {
        create_provisional_identity(&self.app.id, email).unwrap()
    }

    pub fn get_public_identity(&self, identity: &str) -> String {
        get_public_identity(identity).unwrap()
    }

    pub async fn start_anonymous(&self, identity: &str) -> Result<Core, Error> {
        let tanker = Core::new(self.make_options()).await?;
        let status = tanker.start(identity).await?;
        assert_eq!(status, Status::IdentityRegistrationNeeded);

        let key = tanker.generate_verification_key().await?;
        let verif = Verification::VerificationKey(key);
        tanker
            .register_identity(&verif, &VerificationOptions::new())
            .await?;
        assert_eq!(tanker.status(), Status::Ready);

        Ok(tanker)
    }

    pub fn get_oidc_config(&self) -> &OidcConfig {
        &self.config.oidc_config
    }
}

/// Due to the lack of proper async drop, just spawn a whole runtime in a thread
/// Otherwise blocking the current thread could block the _same_ runtime that we're trying
/// to execute our drop future on, which sadly tends to make it wait for a pretty long ever.
fn async_drop_in_thread<F: Future + Send>(future: F) -> F::Output
where
    F::Output: Send,
{
    std::thread::scope(|s| {
        s.spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(future)
        })
        .join()
        .expect("async drop thread join failed")
    })
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Err(err) = async_drop_in_thread(self.admin.delete_app(&self.app.id)) {
            panic!("Error deleting the test app: {err}");
        }
    }
}
