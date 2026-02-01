#![allow(unused_crate_dependencies, clippy::tests_outside_test_module, clippy::redundant_test_prefix, clippy::uninlined_format_args)]
//! Integration tests for server restart endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// POST /v0/server/restart - requires admin auth
#[tokio::test]
async fn test_restart_requires_auth() {
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

// POST /v0/server/restart - non-admin cannot restart
#[tokio::test]
async fn test_restart_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let _admin = server.signup("Admin User", "restartadmin@example.com").await;
    // Second user is NOT admin
    let user = server.signup("Regular User", "restartuser@example.com").await;

    let body = serde_json::json!({});

    let resp = server
        .client
        .post(server.api_url("/v0/server/restart"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Non-admin should be forbidden
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/server/restart - admin can trigger restart with delay
// Note: We don't actually restart the server in tests, just verify the endpoint accepts the request
#[tokio::test]
async fn test_restart_as_admin_with_delay() {
    let server = TestServer::new().await;
    // First user is admin
    let admin = server.signup("Admin User", "adminrestart@example.com").await;

    // Request restart with a long delay so it doesn't actually happen during test
    let body = serde_json::json!({
        "delay": 3600  // 1 hour delay - won't actually trigger during test
    });

    let resp = server
        .client
        .post(server.api_url("/v0/server/restart"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Admin should be able to trigger restart
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}
