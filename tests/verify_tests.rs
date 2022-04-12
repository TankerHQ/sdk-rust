mod identity;

use identity::TestApp;
use serde_json::{json, Value};
use tankersdk::*;

#[tokio::test(flavor = "multi_thread")]
async fn validate_new_device_with_verif_key() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);
    let key = Verification::VerificationKey(tanker.generate_verification_key().await?);
    tanker
        .register_identity(&key, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityVerificationNeeded);
    tanker
        .verify_identity(&key, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn setup_and_use_passphrase() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Beauty In The Ordinary".into());
    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityVerificationNeeded);
    tanker
        .verify_identity(&pass, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn unlock_with_updated_passphrase() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let first_pass = Verification::Passphrase("2564ms".into());
    let second_pass = Verification::Passphrase("light forward".into());

    let verif_options = &VerificationOptions::new();
    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker.register_identity(&first_pass, verif_options).await?;
    tanker
        .set_verification_method(&second_pass, verif_options)
        .await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityVerificationNeeded);
    tanker.verify_identity(&second_pass, verif_options).await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn check_passphrase_is_setup() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Cost of Legacy".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;
    let methods = tanker.get_verification_methods().await?;
    tanker.stop().await?;

    assert!(matches!(*methods, [VerificationMethod::Passphrase]));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn check_email_verif_is_setup() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let email = "cold@in.af".to_string();
    let verif = Verification::Email {
        email: email.clone(),
        verification_code: app.get_email_verification_code(&email).await?,
    };

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    let methods = tanker.get_verification_methods().await?;
    tanker.stop().await?;

    assert_eq!(&methods, &[VerificationMethod::Email(email)]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn check_sms_verif_is_setup() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let phone_number = "+33639982233".to_string();
    let verif = Verification::PhoneNumber {
        phone_number: phone_number.clone(),
        verification_code: app.get_sms_verification_code(&phone_number).await?,
    };

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    let methods = tanker.get_verification_methods().await?;
    tanker.stop().await?;

    assert_eq!(&methods, &[VerificationMethod::PhoneNumber(phone_number)]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn unlock_with_verif_code() -> Result<(), Error> {
    let app = TestApp::get().await;
    let id = &app.create_identity(None);
    let email = "mono@chromat.ic";

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_email_verification_code(email).await?,
    };
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_email_verification_code(email).await?,
    };
    tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn unlock_with_oidc_id_token() -> Result<(), Box<dyn std::error::Error>> {
    let app = TestApp::get().await;
    let oidc = app.get_oidc_config();
    let martine_config = &oidc.users["martine"];
    let martine_identity = app.create_identity(Some(martine_config.email.clone()));

    app.app_update(Some(&oidc.client_id), Some(&oidc.provider), None)
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

    let nonce = tanker.create_oidc_nonce().await?;
    tanker._set_oidc_test_nonce(&nonce).await?;
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    let nonce = tanker.create_oidc_nonce().await?;
    tanker._set_oidc_test_nonce(&nonce).await?;
    tanker.start(&martine_identity).await?;
    assert_eq!(tanker.status(), Status::IdentityVerificationNeeded);
    tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_session_token_with_register_identity() -> Result<(), Error> {
    let app = TestApp::get().await;

    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Cost of Legacy".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    let token = tanker
        .register_identity(&pass, &VerificationOptions::new().with_session_token())
        .await?;
    assert!(token.is_some());
    tanker.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_session_token_with_verify_identity() -> Result<(), Error> {
    let app = TestApp::get().await;

    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("Merge remote-tracking branch 'rust/rust-next'".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;

    let token = tanker
        .verify_identity(&pass, &VerificationOptions::new().with_session_token())
        .await?;
    assert!(token.is_some());
    tanker.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_session_token_with_set_verififcation_method_with_passphrase() -> Result<(), Error> {
    let app = TestApp::get().await;

    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("Phobos".into());
    let pass2 = Verification::Passphrase("Deimos".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;

    let token = tanker
        .set_verification_method(&pass2, &VerificationOptions::new().with_session_token())
        .await?;
    assert!(token.is_some());
    tanker.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_session_token_with_set_verififcation_method_with_email() -> Result<(), Error> {
    let app = TestApp::get().await;

    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("Phobos".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;

    let email = "mono@chromat.ic";

    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_email_verification_code(email).await?,
    };

    let token = tanker
        .set_verification_method(&verif, &VerificationOptions::new().with_session_token())
        .await?;
    assert!(token.is_some());
    tanker.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_session_token_with_set_verififcation_method_with_phone_number() -> Result<(), Error> {
    let app = TestApp::get().await;

    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("Phobos".into());

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;

    let phone_number = "+33639982233";

    let verif = Verification::PhoneNumber {
        phone_number: phone_number.to_owned(),
        verification_code: app.get_sms_verification_code(phone_number).await?,
    };

    let token = tanker
        .set_verification_method(&verif, &VerificationOptions::new().with_session_token())
        .await?;
    assert!(token.is_some());
    tanker.stop().await?;
    Ok(())
}

async fn check_session_token_with_server(
    app: &TestApp,
    public_identity: &str,
    session_token: &str,
    allowed_method: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/verification/session-token", app.trustchaind_url(),);

    let reply: Value = reqwest::Client::new()
        .post(url)
        .json(&json!({
            "app_id": app.id(),
            "auth_token": app.auth_token(),
            "public_identity": public_identity,
            "session_token": session_token,
            "allowed_methods": [{
                "type": allowed_method
            }]
        }))
        .send()
        .await?
        .json()
        .await?;
    Ok(reply["verification_method"].as_str().unwrap().to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn check_session_token_is_valid() -> Result<(), Box<dyn std::error::Error>> {
    let app = TestApp::get().await;

    let tanker = Core::new(app.make_options()).await?;
    let identity = &app.create_identity(None);
    tanker.start(identity).await?;
    let token = tanker
        .register_identity(
            &Verification::Passphrase("Less than three".into()),
            &VerificationOptions::new().with_session_token(),
        )
        .await?
        .unwrap();

    let expected_method = "passphrase";
    let actual_method_used = check_session_token_with_server(
        &app,
        &app.get_public_identity(identity),
        &token,
        expected_method,
    )
    .await?;
    assert_eq!(expected_method, actual_method_used);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn register_fail_with_preverified_email() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let email = "mono@chromat.ic";

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;

    let verif = Verification::PreverifiedEmail(email.into());

    let err = tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await
        .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn register_fail_with_preverified_phone_number() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let phone_number = "+33639982233";

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;

    let verif = Verification::PreverifiedPhoneNumber(phone_number.into());

    let err = tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await
        .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn verify_identity_fail_with_preverified_email() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let email = "mono@chromat.ic";

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);

    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_email_verification_code(email).await?,
    };
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityVerificationNeeded);
    let verif = Verification::PreverifiedEmail(email.into());
    let err = tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await
        .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn verify_identity_fail_with_preverified_phone_number() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let phone_number = "+33639982233";

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);

    let verif = Verification::PhoneNumber {
        phone_number: phone_number.to_owned(),
        verification_code: app.get_sms_verification_code(phone_number).await?,
    };
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);
    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityVerificationNeeded);
    let verif = Verification::PreverifiedPhoneNumber(phone_number.into());
    let err = tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await
        .unwrap_err();

    assert_eq!(err.code(), ErrorCode::InvalidArgument);

    tanker.stop().await
}

#[tokio::test(flavor = "multi_thread")]
async fn set_verification_method_with_preverified_email() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Beauty In The Ordinary".into());
    let email = "mono@chromat.ic";

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    let verif = Verification::PreverifiedEmail(email.into());
    tanker
        .set_verification_method(&verif, &VerificationOptions::new())
        .await?;
    let methods = tanker.get_verification_methods().await?;
    assert_eq!(
        *methods,
        [
            VerificationMethod::PreverifiedEmail(email.to_string()),
            VerificationMethod::Passphrase
        ]
    );

    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    let verif = Verification::Email {
        email: email.to_owned(),
        verification_code: app.get_email_verification_code(email).await?,
    };
    tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    let methods = tanker.get_verification_methods().await?;
    assert_eq!(
        *methods,
        [
            VerificationMethod::Email(email.to_string()),
            VerificationMethod::Passphrase
        ]
    );

    tanker.stop().await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn set_verification_method_with_preverified_phone_number() -> Result<(), Error> {
    let app = TestApp::get().await;
    app.app_update(None, None, Some(true)).await?;
    let id = &app.create_identity(None);
    let pass = Verification::Passphrase("The Beauty In The Ordinary".into());
    let phone_number = "+33639982233".to_string();

    let tanker = Core::new(app.make_options()).await?;
    assert_eq!(tanker.start(id).await?, Status::IdentityRegistrationNeeded);
    tanker
        .register_identity(&pass, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    let verif = Verification::PreverifiedPhoneNumber(phone_number.clone());
    tanker
        .set_verification_method(&verif, &VerificationOptions::new())
        .await?;
    let methods = tanker.get_verification_methods().await?;
    assert_eq!(
        *methods,
        [
            VerificationMethod::PreverifiedPhoneNumber(phone_number.to_string()),
            VerificationMethod::Passphrase
        ]
    );

    tanker.stop().await?;

    let tanker = Core::new(app.make_options()).await?;
    tanker.start(id).await?;
    let verif = Verification::PhoneNumber {
        phone_number: phone_number.clone(),
        verification_code: app.get_sms_verification_code(&phone_number).await?,
    };
    tanker
        .verify_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    let methods = tanker.get_verification_methods().await?;
    assert_eq!(
        *methods,
        [
            VerificationMethod::PhoneNumber(phone_number.to_string()),
            VerificationMethod::Passphrase
        ]
    );

    tanker.stop().await?;

    Ok(())
}
