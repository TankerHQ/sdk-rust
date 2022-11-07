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

#[tokio::test(flavor = "multi_thread")]
async fn tanker_create() -> Result<(), Error> {
    let app = TestApp::get().await;
    let opts = Options::new(
        app.id().to_owned(),
        ":memory:".to_string(),
        ":memory:".to_string(),
    )
    .with_sdk_type("sdk-rust-test".to_string());
    let core = Core::new(opts).await?;
    drop(core);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn tanker_bad_create() {
    let opts = Options::new(
        "bad-app-id".to_string(),
        ":memory:".to_string(),
        ":memory:".to_string(),
    )
    .with_sdk_type("sdk-rust-test".to_string());
    let err = Core::new(opts)
        .await
        .expect_err("The app ID should not be accepted, it's not valid base64");
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
}

#[tokio::test(flavor = "multi_thread")]
async fn start_stop_session() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = Core::new(app.make_options()).await?;
    let status = tanker.start(&app.create_identity(None)).await?;
    assert_eq!(status, Status::IdentityRegistrationNeeded);

    let passphrase = Verification::Passphrase("pass".into());
    tanker
        .register_identity(&passphrase, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    tanker.stop().await?;
    assert_eq!(tanker.status(), Status::Stopped);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
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

const SIMPLE_ENCRYPTION_OVERHEAD: usize = 17;
const SIMPLE_PADDED_ENCRYPTION_OVERHEAD: usize = SIMPLE_ENCRYPTION_OVERHEAD + 1;

#[tokio::test(flavor = "multi_thread")]
async fn padding_opt_auto_by_default() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"my clear data is clear!";
    let length_with_padme = 24;
    let encrypted = tanker.encrypt(plaintext, &Default::default()).await?;

    assert_eq!(
        encrypted.len() - SIMPLE_PADDED_ENCRYPTION_OVERHEAD,
        length_with_padme
    );

    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn padding_opt_auto() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"my clear data is clear!";
    let length_with_padme = 24;
    let options = EncryptionOptions::new().padding_step(Padding::Auto);
    let encrypted = tanker.encrypt(plaintext, &options).await?;

    assert_eq!(
        encrypted.len() - SIMPLE_PADDED_ENCRYPTION_OVERHEAD,
        length_with_padme
    );

    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn padding_opt_disable() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"Chocolate";
    let options = EncryptionOptions::new().padding_step(Padding::Off);
    let encrypted = tanker.encrypt(plaintext, &options).await?;

    assert_eq!(
        encrypted.len() - SIMPLE_ENCRYPTION_OVERHEAD,
        plaintext.len()
    );

    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn padding_opt_enable() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"Chocolate";
    let options = EncryptionOptions::new().padding_step(Padding::with_step(13)?);
    let encrypted = tanker.encrypt(plaintext, &options).await?;

    assert_eq!(
        (encrypted.len() - SIMPLE_PADDED_ENCRYPTION_OVERHEAD) % 13_usize,
        0
    );

    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_padding_step_zero() -> Result<(), Error> {
    let err = Padding::with_step(0).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert_eq!(
        err.message(),
        "Invalid padding step, the value must be >= 2."
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_padding_step_one() -> Result<(), Error> {
    let err = Padding::with_step(1).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert_eq!(
        err.message(),
        "Invalid padding step, the value must be >= 2."
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_device_session_and_encrypt() -> Result<(), Error> {
    let app = TestApp::get().await;

    let tmp_dir = tempfile::Builder::new()
        .prefix("rust-test.")
        .tempdir()
        .expect("Failed to create temp dir for tanker storage");
    let tmp_dir_str = tmp_dir.path().to_str().unwrap().to_owned();
    let options = Options::new(app.id().to_owned(), tmp_dir_str.clone(), tmp_dir_str)
        .with_url(app.url().to_owned())
        .with_sdk_type("sdk-rust-test".to_string());

    let identity = app.create_identity(None);
    let tanker = Core::new(options.clone()).await?;
    let status = tanker.start(&identity).await?;
    assert_eq!(status, Status::IdentityRegistrationNeeded);

    let verif = Verification::E2ePassphrase("12345".into());
    tanker
        .register_identity(&verif, &VerificationOptions::new())
        .await?;
    assert_eq!(tanker.status(), Status::Ready);

    tanker.stop().await?;
    drop(tanker);

    let tanker2 = Core::new(options).await?;
    tanker2.start(&identity).await?;
    assert_eq!(tanker2.status(), Status::Ready);

    let plaintext = b"Nepenthe";
    let encrypted = tanker2.encrypt(plaintext, &Default::default()).await?;
    let decrypted = tanker2.decrypt(&encrypted).await?;
    tanker2.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn share_then_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Rome";
    let encrypted = alice.encrypt(plaintext, &Default::default()).await?;
    let resource_id = alice.get_resource_id(&encrypted)?;

    let options = SharingOptions::new().share_with_users([bob_public_id]);
    alice.share(&[resource_id], &options).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    alice.stop().await?;
    bob.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn share_duplicate_user_id() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Turin";
    let encrypted = alice.encrypt(plaintext, &Default::default()).await?;
    let resource_id = alice.get_resource_id(&encrypted)?;

    let options = SharingOptions::new().share_with_users([bob_public_id.clone(), bob_public_id]);
    alice.share(&[resource_id], &options).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    alice.stop().await?;
    bob.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn encrypt_and_share_then_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Ludwigsburg";
    let options = EncryptionOptions::new().share_with_users([bob_public_id]);
    let encrypted = alice.encrypt(plaintext, &options).await?;
    let decrypted = bob.decrypt(&encrypted).await?;
    alice.stop().await?;
    bob.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn encrypt_no_share_with_self() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let plaintext = b"Eclipse";
    let options = EncryptionOptions::new()
        .share_with_users([bob_public_id])
        .share_with_self(false);
    let encrypted = alice.encrypt(plaintext, &options).await?;

    let _ = bob.decrypt(&encrypted).await.unwrap();
    let err = alice.decrypt(&encrypted).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(err.message().contains("key not found"));

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn share_with_provisional_user() -> Result<(), Error> {
    let message = b"Variable 'message' is never used";
    let app = TestApp::get().await;

    let bob_email = "bob@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);
    let bob_public_id = app.get_public_identity(&bob_provisional);

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let options = EncryptionOptions::new().share_with_users([bob_public_id]);
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
        verification_code: app.get_email_verification_code(&bob_email).await?,
    };
    bob.verify_provisional_identity(&verif).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    assert_eq!(&decrypted, message);

    bob.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn share_with_duplicate_provisional_user() -> Result<(), Error> {
    let message = b"Variable 'message' is never used";
    let app = TestApp::get().await;

    let bob_email = "bob@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);
    let bob_public_id = app.get_public_identity(&bob_provisional);

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let options = EncryptionOptions::new().share_with_users([bob_public_id.clone(), bob_public_id]);
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
        verification_code: app.get_email_verification_code(&bob_email).await?,
    };
    bob.verify_provisional_identity(&verif).await?;

    let decrypted = bob.decrypt(&encrypted).await?;
    assert_eq!(&decrypted, message);

    bob.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn throws_if_identity_is_already_attached() -> Result<(), Error> {
    let app = TestApp::get().await;

    let bob_email = "bob@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);

    let bob = app.start_anonymous(&app.create_identity(None)).await?;
    let attach_result = bob.attach_provisional_identity(&bob_provisional).await?;
    assert_eq!(attach_result.status, Status::IdentityVerificationNeeded);
    assert_eq!(
        attach_result.verification_method,
        Some(VerificationMethod::Email(bob_email.clone()))
    );

    let verif = Verification::Email {
        email: bob_email.clone(),
        verification_code: app.get_email_verification_code(&bob_email).await?,
    };
    bob.verify_provisional_identity(&verif).await?;

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let attach_result = alice.attach_provisional_identity(&bob_provisional).await?;
    assert_eq!(attach_result.status, Status::IdentityVerificationNeeded);
    let verif = Verification::Email {
        email: bob_email.clone(),
        verification_code: app.get_email_verification_code(&bob_email).await?,
    };
    let err = alice.verify_provisional_identity(&verif).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::IdentityAlreadyAttached);

    bob.stop().await?;
    alice.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn attach_provisional_with_single_verif() -> Result<(), Error> {
    let message = b"Variable 'message' is never used";
    let app = TestApp::get().await;

    let bob_email = "bob2@tanker.io".to_owned();
    let bob_provisional = app.create_provisional_identity(&bob_email);
    let bob_public_id = app.get_public_identity(&bob_provisional);

    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let options = EncryptionOptions::new().share_with_users([bob_public_id]);
    let encrypted = alice.encrypt(message, &options).await?;
    alice.stop().await?;

    let bob = Core::new(app.make_options()).await?;
    bob.start(&app.create_identity(None)).await?;
    let verif = Verification::Email {
        email: bob_email.clone(),
        verification_code: app.get_email_verification_code(&bob_email).await?,
    };
    bob.register_identity(&verif, &VerificationOptions::new())
        .await?;

    let attach_result = bob.attach_provisional_identity(&bob_provisional).await?;
    assert_eq!(attach_result.status, Status::Ready);
    assert_eq!(attach_result.verification_method, None);

    let decrypted = bob.decrypt(&encrypted).await?;
    assert_eq!(&decrypted, message);

    bob.stop().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn prehash_password_empty() -> Result<(), Error> {
    let err = Core::prehash_password("").unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn prehash_password_test_vector_1() -> Result<(), Error> {
    let input = "super secretive password";
    let expected = "UYNRgDLSClFWKsJ7dl9uPJjhpIoEzadksv/Mf44gSHI=";
    let result = Core::prehash_password(input)?;
    assert_eq!(result, expected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn prehash_password_test_vector_2() -> Result<(), Error> {
    let input = "test éå 한국어 😃";
    let expected = "Pkn/pjub2uwkBDpt2HUieWOXP5xLn0Zlen16ID4C7jI=";
    let result = Core::prehash_password(input)?;
    assert_eq!(result, expected);
    Ok(())
}
