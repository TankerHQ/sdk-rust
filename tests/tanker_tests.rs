mod identity;

use identity::TestApp;
use tankersdk::*;

#[test]
fn cargo_version() {
    assert!(!Core::version().is_empty())
}

#[test]
fn core_native_version() {
    assert!(!Core::native_version().is_empty())
}

#[tokio::test]
async fn tanker_create() -> Result<(), Error> {
    let app = TestApp::get().await;
    let opts = Options::new(app.id().to_owned(), ":memory:".to_string())
        .with_sdk_type("sdk-rust-test".to_string());
    let core = Core::new(opts).await?;
    drop(core);
    Ok(())
}

#[tokio::test]
async fn tanker_bad_create() {
    let opts = Options::new("bad-app-id".to_string(), ":memory:".to_string())
        .with_sdk_type("sdk-rust-test".to_string());
    let err = Core::new(opts)
        .await
        .expect_err("The app ID should not be accepted, it's not valid base64");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn start_stop_session() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = Core::new(app.make_options()).await?;
    let status = tanker.start(&app.create_identity(None)).await?;
    assert_eq!(status, Status::IdentityRegistrationNeeded);

    let passphrase = Verification::Passphrase("pass".into());
    tanker.register_identity(&passphrase).await?;
    assert_eq!(tanker.status(), Status::Ready);

    tanker.stop().await?;
    assert_eq!(tanker.status(), Status::Stopped);
    Ok(())
}

#[tokio::test]
async fn self_revoke() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    #[allow(deprecated)]
    tanker.revoke_device(&tanker.device_id()?).await?;
    let err = tanker.encrypt(b"F", &Default::default()).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::DeviceRevoked);

    tanker.stop().await
}

#[tokio::test]
async fn has_correct_device_list() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let list = tanker.device_list().await?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].revoked, false);
    assert_eq!(list[0].id, tanker.device_id().unwrap());

    tanker.stop().await
}

#[tokio::test]
async fn encrypt_and_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"Chocolate";
    let encrypted = tanker.encrypt(plaintext, &Default::default()).await?;
    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test]
async fn share_then_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Rome";
    let encrypted = alice.encrypt(plaintext, &Default::default()).await?;
    let resource_id = alice.get_resource_id(&encrypted)?;

    let options = SharingOptions::new().share_with_users(&[bob_public_id]);
    alice.share(&[resource_id], &options).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    alice.stop().await?;
    bob.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test]
async fn encrypt_and_share_then_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Ludwigsburg";
    let options = EncryptionOptions::new().share_with_users(&[bob_public_id]);
    let encrypted = alice.encrypt(plaintext, &options).await?;
    let decrypted = bob.decrypt(&encrypted).await?;
    alice.stop().await?;
    bob.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test]
async fn encrypt_no_share_with_self() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Eclipse";
    let options = EncryptionOptions::new()
        .share_with_users(&[bob_public_id])
        .share_with_self(false);
    let encrypted = alice.encrypt(plaintext, &options).await?;

    let _ = bob.decrypt(&encrypted).await.unwrap();
    let err = alice.decrypt(&encrypted).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(err.message().contains("can't find keys"));

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}

#[tokio::test]
async fn share_with_provisional_user() -> Result<(), Error> {
    let message = b"Variable 'message' is never used";
    let app = TestApp::get().await;

    let bob_email = "bob@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);
    let bob_public_id = app.get_public_identity(&bob_provisional);

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let options = EncryptionOptions::new().share_with_users(&[bob_public_id]);
    let encrypted = alice.encrypt(message, &options).await?;
    alice.stop().await?;

    let bob = app.start_anonymous(&app.create_identity(None)).await?;
    let attach_result = bob.attach_provisional_identity(&bob_provisional).await?;
    assert_eq!(attach_result.status, Status::IdentityVerificationNeeded);
    assert_eq!(
        attach_result.verification_method,
        Some(VerificationMethod::Email(bob_email.clone()))
    );

    let verif = Verification::Email {
        email: bob_email.clone(),
        verification_code: app.get_verification_code(&bob_email).await?,
    };
    bob.verify_provisional_identity(&verif).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    assert_eq!(&decrypted, message);

    bob.stop().await?;
    Ok(())
}

#[tokio::test]
async fn attach_provisional_with_single_verif() -> Result<(), Error> {
    let message = b"Variable 'message' is never used";
    let app = TestApp::get().await;

    let bob_email = "bob2@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);
    let bob_public_id = app.get_public_identity(&bob_provisional);

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let options = EncryptionOptions::new().share_with_users(&[bob_public_id]);
    let encrypted = alice.encrypt(message, &options).await?;
    alice.stop().await?;

    let bob = Core::new(app.make_options()).await?;
    bob.start(&app.create_identity(None)).await?;
    let verif = Verification::Email {
        email: bob_email.clone(),
        verification_code: app.get_verification_code(&bob_email).await?,
    };
    bob.register_identity(&verif).await?;

    let attach_result = bob.attach_provisional_identity(&bob_provisional).await?;
    assert_eq!(attach_result.status, Status::Ready);
    assert_eq!(attach_result.verification_method, None);

    let decrypted = bob.decrypt(&encrypted).await?;
    assert_eq!(&decrypted, message);

    bob.stop().await?;
    Ok(())
}

#[tokio::test]
async fn prehash_password_empty() -> Result<(), Error> {
    let err = Core::prehash_password("").unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    Ok(())
}

#[tokio::test]
async fn prehash_password_test_vector_1() -> Result<(), Error> {
    let input = "super secretive password";
    let expected = "UYNRgDLSClFWKsJ7dl9uPJjhpIoEzadksv/Mf44gSHI=";
    let result = Core::prehash_password(input)?;
    assert_eq!(result, expected);
    Ok(())
}

#[tokio::test]
async fn prehash_password_test_vector_2() -> Result<(), Error> {
    let input = "test éå 한국어 😃";
    let expected = "Pkn/pjub2uwkBDpt2HUieWOXP5xLn0Zlen16ID4C7jI=";
    let result = Core::prehash_password(input)?;
    assert_eq!(result, expected);
    Ok(())
}
