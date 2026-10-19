#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the server health endpoint.

use bencher_api_tests::{LogCapture, TestServer};
use http::StatusCode;

// GET /v0/server/health - public, no auth required
#[tokio::test]
async fn health_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/health"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/server/health - a database the server cannot read fails the check with a 503,
// even on a connection that was already open; the anonymous caller learns nothing about
// the database, and the server log keeps the error
#[tokio::test]
async fn health_get_unreadable_database() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let health = async || {
        server
            .client
            .get(server.api_url("/v0/server/health"))
            .send()
            .await
            .expect("Request failed")
    };
    assert_eq!(health().await.status(), StatusCode::OK);

    std::fs::write(server.db_path(), b"not a database").expect("Failed to overwrite the database");

    let resp = health().await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = resp.text().await.expect("Failed to read the body");
    assert!(!body.contains("not a database"), "{body}");
    assert!(!body.contains("github.com"), "{body}");
    assert!(
        capture
            .lines()
            .iter()
            .any(|line| line.contains("not a database")),
        "the server log keeps the error"
    );
}
