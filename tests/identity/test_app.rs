#![allow(dead_code)] // This module is compiled per-test. Not all tests will use all functions!

mod config;
use config::{Config, OIDCConfig};

use super::Admin;
use super::App;
use crate::identity::{create_identity, create_provisional_identity, get_public_identity};
use double_checked_cell_async::DoubleCheckedCell;
use futures::executor::block_on;
use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::Rng;
use tankersdk::{Core, Error, LogRecordLevel, Options, Status, Verification};

lazy_static! {
    static ref GLOBAL_APP: DoubleCheckedCell<TestApp> = DoubleCheckedCell::new();
}

pub struct TestApp {
    config: Config,
    admin: Admin,
    app: App,
}

impl TestApp {
    pub async fn get() -> &'static Self {
        Core::set_log_handler(Box::new(|record| {
            if record.level >= LogRecordLevel::Warning {
                println!(
                    "[{} {}#{}:{}] {}",
                    record.level, record.category, record.file, record.line, record.message
                )
            }
        }));
        GLOBAL_APP.get_or_init(TestApp::new()).await
    }

    pub fn make_options(&self) -> Options {
        Options::new(self.id().to_owned(), ":memory:".to_owned())
            .with_url(self.config.api_url.clone())
            .with_sdk_type("sdk-rust-test".to_string())
    }

    async fn new() -> Self {
        let config = Config::new();
        let admin = Admin::new(
            config.admin_url.clone(),
            config.id_token.clone(),
            config.api_url.clone(),
            config.trustchain_url.clone(),
        )
        .await
        .unwrap();
        let app = admin.create_app("rust-test", true).await.unwrap();
        Self { config, admin, app }
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub async fn get_verification_code(&self, email: &str) -> Result<String, Error> {
        self.app.get_verification_code(email).await
    }

    pub async fn app_update(
        &self,
        oidc_client_id: Option<&str>,
        oidc_provider: Option<&str>,
    ) -> Result<(), Error> {
        self.admin
            .app_update(&self.app.id, oidc_client_id, oidc_provider)
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
        let status = tanker.start(&identity).await?;
        assert_eq!(status, Status::IdentityRegistrationNeeded);

        let key = tanker.generate_verification_key().await?;
        let verif = Verification::VerificationKey(key);
        tanker.register_identity(&verif).await?;
        assert_eq!(tanker.status(), Status::Ready);

        Ok(tanker)
    }

    pub fn get_oidc_config(&self) -> &OIDCConfig {
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
