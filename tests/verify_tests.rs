mod identity;

use identity::TestApp;
use serde_json::{json, Value};
use tankersdk::*;

#[tokio::test]
async fn validate_new_device_with_verif_key() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(&id).await?, Status::IdentityRegistrationNeeded);
    let key = Verification::VerificationKey(tanker.generate_verification_key().await?);
    tanker.register_identity(&key).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(&id).await?, Status::IdentityVerificationNeeded);
    tanker.verify_identity(&key).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test]
async fn setup_and_use_passphrase() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Beauty In The Ordinary".into());
    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(&id).await?, Status::IdentityRegistrationNeeded);
    tanker.register_identity(&pass).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(&id).await?, Status::IdentityVerificationNeeded);
    tanker.verify_identity(&pass).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test]
async fn unlock_with_updated_passphrase() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let first_pass = Verification::Passphrase("2564ms".into());
    let second_pass = Verification::Passphrase("light forward".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&id).await?;
    tanker.register_identity(&first_pass).await?;
    tanker.set_verification_method(&second_pass).await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(&id).await?, Status::IdentityVerificationNeeded);
    tanker.verify_identity(&second_pass).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test]
async fn check_passphrase_is_setup() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Cost of Legacy".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&id).await?;
    tanker.register_identity(&pass).await?;
    let methods = tanker.get_verification_methods().await?;
    tanker.stop().await?;

    assert!(matches!(*methods, [VerificationMethod::Passphrase]));
    Ok(())
}

#[tokio::test]
async fn check_email_verif_is_setup() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let email = "cold@in.af".to_string();
    let verif = Verification::Email {
        email: email.clone(),
        verification_code: app.get_verification_code(&email).await?,
    };

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&id).await?;
    tanker.register_identity(&verif).await?;
    let methods = tanker.get_verification_methods().await?;
    tanker.stop().await?;

    assert_eq!(&methods, &[VerificationMethod::Email(email)]);
    Ok(())
}

#[tokio::test]
async fn unlock_with_verif_code() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let email = "mono@chromat.ic";

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&id).await?;
    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_verification_code(&email).await?,
    };
    tanker.register_identity(&verif).await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&id).await?;
    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_verification_code(&email).await?,
    };
    tanker.verify_identity(&verif).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test]
async fn unlock_with_oidc_id_token() -> Result<(), Box<dyn std::error::Error>> {
    let app = TestApp::get().await;
    let oidc = app.get_oidc_config();
    let martine_config = &oidc.users["martine"];
    let martine_identity = app.create_identity(Some(martine_config.email.clone()));

    app.app_update(Some(&oidc.client_id), Some(&oidc.provider))
        .await?;

    let client = reqwest::Client::new();
    let response = client
        .post("https://www.googleapis.com/oauth2/v4/token")
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": &martine_config.refresh_token,
            "client_id": &oidc.client_id,
            "client_secret": &oidc.client_secret,
        }))
        .send()
        .await?;
    let reply: Value = response.json().await?;
    let oidc_token = reply["id_token"].as_str().unwrap();

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&martine_identity).await?;
    let verif = Verification::OIDCIDToken(oidc_token.to_owned());
    tanker.register_identity(&verif).await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(&martine_identity).await?;
    assert_eq!(tanker.status(), Status::IdentityVerificationNeeded);
    tanker.verify_identity(&verif).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;
    Ok(())
}
