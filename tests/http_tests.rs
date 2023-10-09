mod identity;

use axum::body::Body;
use axum::handler::Handler;
use axum::http::{Request, StatusCode};
use axum::Router;
use identity::TestApp;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tankersdk::*;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::timeout;

async fn spawn_test_http_server<T>(svc: impl Handler<T, (), Body> + Clone + Send + 'static) -> u16 {
    let handler_thunk = |req: Request<Body>| async {
        svc.call(req, ()).await;
        (
            StatusCode::NOT_FOUND,
            "Test HTTP server cannot handle real requests".to_string(),
        )
    };

    let app = Router::new().fallback(handler_thunk);
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    let addr = server.local_addr();
    std::mem::drop(spawn(server));
    addr.port()
}

#[tokio::test(flavor = "multi_thread")]
async fn simple_http_request() -> Result<(), Error> {
    let (wait_tx, mut wait_rx) = tokio::sync::mpsc::channel(1);
    let wait_tx = Arc::new(wait_tx);

    // Async closures are unstable, so enjoy the move async move syntax :)
    let port = spawn_test_http_server(move || async move {
        let _ = wait_tx.send(()).await;
    })
    .await;

    let app = TestApp::get().await;
    let opts = Options::new(
        app.id().to_owned(),
        ":memory:".to_string(),
        ":memory:".to_string(),
    )
    .with_url(format!("http://127.0.0.1:{port}"))
    .with_sdk_type("sdk-rust-test".to_string());
    let core = Core::new(opts).await?;
    core.start(&app.create_identity(None))
        .await
        .expect_err("Shouldn't have started successfully!");

    timeout(Duration::from_secs(2), wait_rx.recv())
        .await
        .expect("Failed while waiting for the HTTP request");

    drop(core);
    Ok(())
}

// We test Core with async-std, but our tests still need a tokio runtime (admin API & axum server)
#[tokio::main(flavor = "multi_thread")]
async fn run_in_tokio<F, T>(cb: impl FnOnce() -> F) -> T
where
    F: Future<Output = T>,
{
    cb().await
}

// Our http client needs a tokio runtime, but if our users are using async-std (some are!),
// we need to make sure we start our own runtime correctly and don't just panic
#[async_std::test]
async fn async_std_http_request() -> Result<(), Error> {
    let (wait_tx, mut wait_rx) = tokio::sync::mpsc::channel(1);
    let wait_tx = Arc::new(wait_tx);

    let port = run_in_tokio(|| async {
        spawn_test_http_server(move || async move {
            let _ = wait_tx.send(()).await;
        })
        .await
    });

    let app = run_in_tokio(|| async { TestApp::get().await });
    let opts = Options::new(
        app.id().to_owned(),
        ":memory:".to_string(),
        ":memory:".to_string(),
    )
    .with_url(format!("http://127.0.0.1:{port}"))
    .with_sdk_type("sdk-rust-test".to_string());
    let core = Core::new(opts).await?;
    core.start(&app.create_identity(None))
        .await
        .expect_err("Shouldn't have started successfully!");

    async_std::future::timeout(Duration::from_secs(2), wait_rx.recv())
        .await
        .expect("Failed while waiting for the HTTP request");

    drop(core);
    run_in_tokio(|| async { drop(app) }); // The drop does an HTTP request to delete the app!
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn request_cancellation() -> Result<(), Error> {
    let (req_received_tx, mut req_received_rx) = tokio::sync::mpsc::channel(1);
    let req_received_tx = Arc::new(req_received_tx);
    let (tanker_cancelled_tx, tanker_cancelled_rx) = tokio::sync::mpsc::channel(1);
    let tanker_cancelled_rx = Arc::new(Mutex::new(tanker_cancelled_rx));
    let (request_finished_tx, mut request_finished_rx) = tokio::sync::mpsc::channel(1);
    let request_finished_tx = Arc::new(Mutex::new(Some(request_finished_tx)));

    let port = spawn_test_http_server(move || async move {
        // We take the only sender holding the channel open. If we drop it, the rx will receive None
        let request_finished_tx = request_finished_tx.lock().await.take().unwrap();

        // Tell the test to drop Core now that native is blocked in the middle of this HTTP request
        req_received_tx.send(()).await.unwrap();

        // Tanker has been dropped, so the Rust client should cancel and close the HTTP connection,
        // which causes our test server to drop this callback's whole async task
        // Because we get dropped while sleeping, request_finished_tx's channel will close with None
        tanker_cancelled_rx.lock().await.recv().await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;

        // If we reach this point, the request hasn't been cancelled, tell the other side...
        request_finished_tx.send(()).await.unwrap();
    })
    .await;

    let app = TestApp::get().await;
    let ident = app.create_identity(None);
    let opts = Options::new(
        app.id().to_owned(),
        ":memory:".to_string(),
        ":memory:".to_string(),
    )
    .with_url(format!("http://127.0.0.1:{port}"))
    .with_sdk_type("sdk-rust-test".to_string());

    let core_future = spawn(async move {
        let core = Core::new(opts).await.unwrap();
        core.start(&ident).await.unwrap();
    });

    // Wait until Tanker is in the middle of calling our test HTTP server
    req_received_rx
        .recv()
        .await
        .expect("Failed to wait for server request");

    // Dropping tanker should cause the HTTP request to be cancelled client-side,
    // which will propagate to the server's callback because the connection will close
    core_future.abort();
    tanker_cancelled_tx.send(()).await.unwrap();

    // The sender's task should have been cancelled after we dropped core
    // If we receive a value, the request finished without cancellation :(
    assert_eq!(request_finished_rx.recv().await, None);

    Ok(())
}
