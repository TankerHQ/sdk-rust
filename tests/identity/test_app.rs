#![allow(dead_code)] // This module is compiled per-test. Not all tests will use all functions!

mod config;
use config::{Config, OidcConfig};

use super::Admin;
use super::App;
use crate::identity::{create_identity, create_provisional_identity, get_public_identity};
use futures::executor::block_on;
use rand::distributions::Alphanumeric;
use rand::Rng;
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
        oidc_client_id: Option<&str>,
        oidc_provider: Option<&str>,
        preverified_verification: Option<bool>,
    ) -> Result<(), Error> {
        self.admin
            .app_update(
                &self.app.id,
                oidc_client_id,
                oidc_provider,
                preverified_verification,
            )
            .await
    }

    pub fn create_identity(&self, email: Option<String>) -> String {
        let user_id = email.unwrap_or_else(|| {
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
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

impl Drop for TestApp {
    fn drop(&mut self) {
        // NOTE: Log and ignore errors, there's nothing we can do
        let result = block_on(self.admin.delete_app(&self.app.id));
        if let Err(err) = result {
            eprintln!("Error deleting the test app: {}", err);
        }
    }
}
