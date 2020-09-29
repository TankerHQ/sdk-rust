mod identity;

use futures::io::ErrorKind;
use futures::{AsyncRead, AsyncReadExt};
use identity::TestApp;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Context, Poll};
use tankersdk::{Error, ErrorCode};

#[tokio::test]
async fn encrypt_stream_and_decrypt() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"Chocolate";
    let mut encrypted_stream = tanker
        .encrypt_stream(plaintext as &[u8], &Default::default())
        .await?;
    let mut encrypted = Vec::new();
    encrypted_stream.read_to_end(&mut encrypted).await.unwrap();
    let decrypted = tanker.decrypt(&encrypted).await?;
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test]
async fn encrypt_and_decrypt_stream() -> Result<(), futures::io::Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let plaintext = b"Chocolate";
    let encrypted = tanker.encrypt(plaintext, &Default::default()).await?;
    let mut decrypted_stream = tanker.decrypt_stream(encrypted.as_slice()).await?;
    let mut decrypted = Vec::new();
    decrypted_stream.read_to_end(&mut decrypted).await.unwrap();
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[tokio::test]
async fn encrypt_stream_and_decrypt_stream() -> Result<(), futures::io::Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    // We want to stress the system a bit here. Tanker makes 1MB chunks, force at least 2 chunks
    let plaintext = b"8 bytes ".repeat(1024 * 1024);
    let encrypted_stream = tanker
        .encrypt_stream(plaintext.as_slice(), &Default::default())
        .await?;
    let mut decrypted_stream = tanker.decrypt_stream(encrypted_stream).await?;
    let mut decrypted = Vec::new();
    decrypted_stream.read_to_end(&mut decrypted).await.unwrap();
    tanker.stop().await?;

    assert_eq!(decrypted, plaintext);
    Ok(())
}

/// Stream that can be read for n bytes, then it will err
struct ErrorAfter {
    n: usize,
}

impl AsyncRead for ErrorAfter {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.n == 0 {
            return Poll::Ready(Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "denied",
            )));
        }
        let to_read = min(buf.len(), self.n);
        self.n -= to_read;
        Poll::Ready(Ok(to_read))
    }
}

#[tokio::test]
async fn encrypt_stream_with_error() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let mut encrypted_stream = tanker
        .encrypt_stream(ErrorAfter { n: 1000 }, &Default::default())
        .await?;
    let mut encrypted = Vec::new();
    let result = encrypted_stream
        .read_to_end(&mut encrypted)
        .await
        .map_err(|e| e.kind());
    tanker.stop().await?;

    assert_eq!(result, Err(ErrorKind::PermissionDenied));

    Ok(())
}

#[tokio::test]
async fn decrypt_stream_with_early_error() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    let result = tanker.decrypt_stream(ErrorAfter { n: 0 }).await;
    tanker.stop().await?;

    let error = result.err().unwrap();
    assert_eq!(error.code(), ErrorCode::IoError);
    let source = (&error as &dyn std::error::Error)
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(source.kind(), ErrorKind::PermissionDenied);
    Ok(())
}

#[tokio::test]
async fn decrypt_stream_with_tanker_error() -> Result<(), Error> {
    let app = TestApp::get().await;
    let tanker = app.start_anonymous(&app.create_identity(None)).await?;

    // We want a long enough stream so that it fails in the middle of the decrypt,
    // not right away during the decrypt_stream.
    let plaintext = b"2b".repeat(1024 * 1024);
    let encrypted = tanker
        .encrypt(plaintext.as_slice(), &Default::default())
        .await?;
    // Truncate the buffer so that decryption fails at the end.
    let encrypted = &encrypted.as_slice()[0..encrypted.len() - 1];
    let mut decrypted_stream = tanker.decrypt_stream(encrypted).await?;
    let mut decrypted = Vec::new();
    let result = decrypted_stream.read_to_end(&mut decrypted).await;
    tanker.stop().await?;

    let error = result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Other);
    let source = error.into_inner().unwrap();
    let source = source.as_ref().downcast_ref::<Error>().unwrap();
    assert_eq!(source.code(), ErrorCode::DecryptionFailed);
    Ok(())
}
