#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix,
    clippy::uninlined_format_args
)]
//! Integration tests for server config endpoints.

use bencher_api_tests::TestServer;
use bencher_json::system::config::JsonConsole;
use http::StatusCode;

// GET /v0/server/config/console - public endpoint
#[tokio::test]
async fn test_config_console_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config/console"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let console: JsonConsole = resp.json().await.expect("Failed to parse response");
    // Console URL should be set
    assert!(!console.url.as_ref().is_empty());
}

// GET /v0/server/config - requires admin auth
#[tokio::test]
async fn test_config_get_requires_auth() {
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

// GET /v0/server/config - admin can view config
#[tokio::test]
async fn test_config_get_as_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let admin = server.signup("Admin User", "configadmin@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    // Admin should be able to view config
    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/server/config - non-admin cannot view config
#[tokio::test]
async fn test_config_get_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let _admin = server.signup("Admin User", "admin@example.com").await;
    // Second user is NOT admin
    let user = server.signup("Regular User", "regular@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Non-admin should be forbidden
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
