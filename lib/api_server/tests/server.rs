//! Integration tests for server endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonApiVersion, JsonSpec};
use http::StatusCode;

// GET /v0/server/version - public
#[tokio::test]
async fn test_server_version() {
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

// GET /v0/server/spec - public
#[tokio::test]
async fn test_server_spec() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/spec"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: JsonSpec = resp.json().await.expect("Failed to parse response");
    // Check that the spec has a version (OpenAPI spec)
    assert!(body.version().is_some());
}

// GET / - root path
#[tokio::test]
async fn test_root_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/"))
        .send()
        .await
        .expect("Request failed");

    // Root returns 200 OK with empty body or redirect
    assert!(resp.status().is_success() || resp.status().is_redirection());
}

// POST / - root path (only available with plus feature)
#[tokio::test]
async fn test_root_post() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/"))
        .send()
        .await
        .expect("Request failed");

    // Root POST is only available with plus feature
    // Without plus, it returns 405 Method Not Allowed or 404 Not Found
    #[cfg(feature = "plus")]
    assert!(resp.status().is_success() || resp.status().is_redirection());
    #[cfg(not(feature = "plus"))]
    assert!(resp.status().is_client_error());
}

// GET /v0/server/config/console - public
#[tokio::test]
async fn test_server_config_console() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config/console"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/server/config - requires admin auth
#[tokio::test]
async fn test_server_config_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .send()
        .await
        .expect("Request failed");

    // Should require authentication
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/server/restart - requires admin auth
#[tokio::test]
async fn test_server_restart_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/server/restart"))
        .send()
        .await
        .expect("Request failed");

    // Should require authentication
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/server/backup - requires admin auth
#[tokio::test]
async fn test_server_backup_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/server/backup"))
        .send()
        .await
        .expect("Request failed");

    // Should require authentication
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
