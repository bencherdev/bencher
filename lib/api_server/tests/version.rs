#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix,
    clippy::uninlined_format_args
)]
//! Integration tests for server version endpoint.

use bencher_api_tests::TestServer;
use bencher_json::JsonApiVersion;
use http::StatusCode;

// GET /v0/server/version - public, no auth required
#[tokio::test]
async fn test_version_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/version"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: JsonApiVersion = resp.json().await.expect("Failed to parse response");
    assert!(!body.version.is_empty());
}

// Verify version format is semver-like
#[tokio::test]
async fn test_version_format() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/version"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: JsonApiVersion = resp.json().await.expect("Failed to parse response");
    // Version should contain dots (semver format)
    assert!(
        body.version.contains('.'),
        "Version should be semver format"
    );
}
