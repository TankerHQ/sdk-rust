mod identity;

use futures::AsyncReadExt;
use identity::TestApp;
use tankersdk::*;

#[tokio::test]
async fn open_close_enc_sess() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let sess = tanker
        .create_encryption_session(&Default::default())
        .await?;
    drop(sess);
    Ok(())
}

#[tokio::test]
async fn share_with_enc_sess() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let data = b"La Pleiade";
    let options = EncryptionOptions::new().share_with_users(&[bob_public_id]);
    let sess = alice.create_encryption_session(&options).await?;
    let encrypted = sess.encrypt(data).await?;
    assert_eq!(bob.decrypt(&encrypted).await?, data);
    Ok(())
}

#[tokio::test]
async fn encrypt_stream_with_enc_sess() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_public_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let data = b"La Comedie Humaine";
    let options = EncryptionOptions::new().share_with_users(&[bob_public_id]);
    let sess = alice.create_encryption_session(&options).await?;
    let encrypted = sess.encrypt_stream(data as &[u8]).await?;

    let mut decrypted_stream = bob.decrypt_stream(encrypted).await?;
    let mut decrypted = Vec::new();
    decrypted_stream.read_to_end(&mut decrypted).await.unwrap();
    assert_eq!(decrypted, data);
    Ok(())
}

#[tokio::test]
async fn resource_id_of_enc_sess_matches_ciphertext() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let sess = tanker
        .create_encryption_session(&Default::default())
        .await?;
    let sess_res_id = sess.get_resource_id();
    let ciphertext = sess.encrypt(b"Les Rougon-Macquart").await?;
    let cipher_res_id = tanker.get_resource_id(&ciphertext)?;
    assert_eq!(sess_res_id, cipher_res_id);
    Ok(())
}

#[tokio::test]
async fn resource_id_of_different_enc_sess_are_different() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob = app.start_anonymous(&app.create_identity(None)).await?;

    let alice_sess = alice.create_encryption_session(&Default::default()).await?;
    let bob_sess = bob.create_encryption_session(&Default::default()).await?;

    assert_ne!(alice_sess.get_resource_id(), bob_sess.get_resource_id());
    Ok(())
}
