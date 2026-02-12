#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner token rotation endpoint.

use bencher_api_tests::TestServer;
use bencher_json::JsonRunnerToken;
use futures_concurrency::future::Join as _;
use http::StatusCode;

// POST /v0/runners/{runner}/token - admin can rotate token
#[tokio::test]
async fn token_rotate_as_admin() {
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
async fn token_rotate_forbidden_for_non_admin() {
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
async fn token_rotate_by_slug() {
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
async fn token_rotate_not_found() {
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

// POST /v0/runners/{runner}/token - concurrent rotation yields two different tokens
#[tokio::test]
async fn concurrent_token_rotation() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenadmin5@example.com").await;

    // Create a runner
    let body = serde_json::json!({ "name": "Concurrent Rotate Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let original: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    let original_str: String = original.token.as_ref().to_owned();

    // Two concurrent rotations
    let url = server.api_url(&format!("/v0/runners/{}/token", original.uuid));
    let bearer = server.bearer(&admin.token);
    let client = &server.client;

    let (resp1, resp2) = (
        async {
            client
                .post(&url)
                .header("Authorization", &bearer)
                .send()
                .await
                .expect("Request 1 failed")
        },
        async {
            client
                .post(&url)
                .header("Authorization", &bearer)
                .send()
                .await
                .expect("Request 2 failed")
        },
    )
        .join()
        .await;

    assert_eq!(resp1.status(), StatusCode::CREATED);
    assert_eq!(resp2.status(), StatusCode::CREATED);

    let token1: JsonRunnerToken = resp1.json().await.expect("Failed to parse response 1");
    let token2: JsonRunnerToken = resp2.json().await.expect("Failed to parse response 2");

    let t1: &str = token1.token.as_ref();
    let t2: &str = token2.token.as_ref();

    // Both tokens should differ from the original
    assert_ne!(t1, original_str, "Token 1 should differ from original");
    assert_ne!(t2, original_str, "Token 2 should differ from original");

    // The two tokens should differ from each other
    assert_ne!(
        t1, t2,
        "Concurrent rotations should produce different tokens"
    );

    // Verify only one of the two tokens works for auth (the last writer wins)
    // We can't predict which one, but exactly one should authenticate.
    // We just verify both have the correct prefix and length.
    assert!(t1.starts_with("bencher_runner_"));
    assert!(t2.starts_with("bencher_runner_"));
    assert_eq!(t1.len(), api_runners::RUNNER_TOKEN_LENGTH);
    assert_eq!(t2.len(), api_runners::RUNNER_TOKEN_LENGTH);
}

// After token rotation, the old token should be rejected.
#[tokio::test]
async fn old_token_rejected_after_rotation() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "tokenold@example.com").await;

    // Create a runner and save the original token
    let body = serde_json::json!({ "name": "Old Token Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let original: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    let original_token: String = original.token.as_ref().to_owned();

    // Rotate the token
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/token", original.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let new: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    let new_token: String = new.token.as_ref().to_owned();

    // Old token should be rejected on the claim endpoint
    let claim_body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", original.uuid)))
        .header("Authorization", format!("Bearer {original_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Old token should be rejected after rotation"
    );

    // New token should work
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", original.uuid)))
        .header("Authorization", format!("Bearer {new_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "New token should authenticate successfully"
    );
}
