#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for runner token rotation endpoint.

use bencher_api_tests::TestServer;
use bencher_json::JsonRunnerToken;
use http::StatusCode;

// POST /v0/runners/{runner}/token - admin can rotate token
#[tokio::test]
async fn test_token_rotate_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenadmin@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Token Rotate Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let original_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Rotate token
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/token", original_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let new_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // UUID should be the same
    assert_eq!(new_token.uuid, original_token.uuid);
    // Token should be different
    let original_str: &str = original_token.token.as_ref();
    let new_str: &str = new_token.token.as_ref();
    assert_ne!(original_str, new_str);
    // New token should start with prefix
    assert!(new_str.starts_with("bencher_runner_"));
}

// POST /v0/runners/{runner}/token - non-admin cannot rotate token
#[tokio::test]
async fn test_token_rotate_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenadmin2@example.com").await;
    let user = server.signup("User", "tokenuser@example.com").await;

    // Create a runner as admin
    let body = serde_json::json!({
        "name": "Token Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Non-admin tries to rotate
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/token", runner_token.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/token - rotate by slug
#[tokio::test]
async fn test_token_rotate_by_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenadmin3@example.com").await;

    // Create a runner with a slug
    let body = serde_json::json!({
        "name": "Token Slug Runner",
        "slug": "token-slug-runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let original_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Rotate token by slug
    let resp = server
        .client
        .post(server.api_url("/v0/runners/token-slug-runner/token"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let new_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(new_token.uuid, original_token.uuid);
}

// POST /v0/runners/{runner}/token - not found for invalid runner
#[tokio::test]
async fn test_token_rotate_not_found() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenadmin4@example.com").await;

    let resp = server
        .client
        .post(server.api_url("/v0/runners/nonexistent-runner/token"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
