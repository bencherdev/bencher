#![allow(unused_crate_dependencies, clippy::tests_outside_test_module, clippy::redundant_test_prefix, clippy::uninlined_format_args)]
//! Integration tests for auth accept (invite) endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// POST /v0/auth/accept - invalid invite token
#[tokio::test]
async fn test_accept_invalid_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "invite": "invalid-token"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invalid token should fail
    assert!(resp.status().is_client_error());
}

// POST /v0/auth/accept - empty invite token
#[tokio::test]
async fn test_accept_empty_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "invite": ""
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert!(resp.status().is_client_error());
}

// POST /v0/auth/accept - missing invite field
#[tokio::test]
async fn test_accept_missing_invite() {
    let server = TestServer::new().await;

    let body = serde_json::json!({});

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
