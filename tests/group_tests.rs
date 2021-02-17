mod identity;

use identity::TestApp;
use std::iter;
use tankersdk::*;

#[tokio::test(threaded_scheduler)]
async fn cannot_create_empty_group() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let err = tanker
        .create_group(iter::empty::<&str>())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidArgument);
    assert!(err.message().contains("empty group"));

    tanker.stop().await?;
    Ok(())
}

#[tokio::test(threaded_scheduler)]
async fn create_valid_group() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice = app.start_anonymous(&app.create_identity(None)).await?;
    let bob_id = app.create_identity(None);
    let bob_pub_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let group_id = alice.create_group(&[&bob_pub_id]).await?;
    assert!(!group_id.is_empty());

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}

#[tokio::test(threaded_scheduler)]
async fn encrypt_and_share_with_external_group() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice_id = app.create_identity(None);
    let alice = app.start_anonymous(&alice_id).await?;
    let bob_id = app.create_identity(None);
    let bob_pub_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let group_id = bob.create_group(&[&bob_pub_id]).await?;

    let message: &[u8] = b"Sawdust. A byproduct.";
    let options = EncryptionOptions::new().share_with_groups(&[group_id]);
    let encrypted = alice.encrypt(message, &options).await?;

    assert_eq!(bob.decrypt(&encrypted).await?, message);

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}

#[tokio::test(threaded_scheduler)]
async fn share_with_external_group() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice_id = app.create_identity(None);
    let alice_pub_id = app.get_public_identity(&alice_id);
    let alice = app.start_anonymous(&alice_id).await?;
    let bob_id = app.create_identity(None);
    let bob = app.start_anonymous(&bob_id).await?;

    let group_id = alice.create_group(&[&alice_pub_id]).await?;

    let msg: &[u8] = b"This week included a major speedup on optimized builds of real-world crates";
    let encrypted = bob.encrypt(msg, &Default::default()).await?;
    let resource_id = bob.get_resource_id(&encrypted)?;

    let options = SharingOptions::new().share_with_groups(&[group_id]);
    bob.share(&[resource_id], &options).await?;

    assert_eq!(alice.decrypt(&encrypted).await?, msg);

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}

#[tokio::test(threaded_scheduler)]
async fn add_member_to_group() -> Result<(), Error> {
    let app = TestApp::get().await;
    let alice_id = app.create_identity(None);
    let alice_pub_id = app.get_public_identity(&alice_id);
    let alice = app.start_anonymous(&alice_id).await?;
    let bob_id = app.create_identity(None);
    let bob_pub_id = app.get_public_identity(&bob_id);
    let bob = app.start_anonymous(&bob_id).await?;

    let group_id = alice.create_group(&[&alice_pub_id]).await?;

    let msg = "Für wenst'd've hätten wir Hunger?".as_bytes();
    let encrypted = alice.encrypt(msg, &Default::default()).await?;
    let resource_id = alice.get_resource_id(&encrypted)?;

    let options = SharingOptions::new().share_with_groups(&[&group_id]);
    alice.share(&[resource_id], &options).await?;

    alice.update_group_members(&group_id, &[bob_pub_id]).await?;

    assert_eq!(bob.decrypt(&encrypted).await?, msg);

    alice.stop().await?;
    bob.stop().await?;
    Ok(())
}
