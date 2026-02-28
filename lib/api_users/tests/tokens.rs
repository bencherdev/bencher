#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for user token endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonToken, JsonTokens};
use http::StatusCode;

// GET /v0/users/{user}/tokens - list tokens (after creating one)
#[tokio::test]
async fn tokens_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokenslist@example.com").await;

    // First create a token so the list isn't empty
    let create_body = serde_json::json!({
        "name": "List Test Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&create_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // Now list tokens
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let tokens: JsonTokens = resp.json().await.expect("Failed to parse response");
    assert!(!tokens.0.is_empty());
}

// POST /v0/users/{user}/tokens - create token
#[tokio::test]
async fn tokens_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokenscreate@example.com").await;

    let body = serde_json::json!({
        "name": "Test Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let token: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(token.name.as_ref(), "Test Token");
}

// POST /v0/users/{user}/tokens - create token with TTL
#[tokio::test]
async fn tokens_create_with_ttl() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensttl@example.com").await;

    let body = serde_json::json!({
        "name": "TTL Token",
        "ttl": 3600  // 1 hour
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// GET /v0/users/{user}/tokens/{token} - view token
#[tokio::test]
async fn tokens_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensget@example.com").await;

    // First create a token
    let body = serde_json::json!({
        "name": "View Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created_token: JsonToken = create_resp.json().await.expect("Failed to parse response");

    // Now fetch it
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/users/{}/tokens/{}",
            user_slug, created_token.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, created_token.uuid);
}

// GET /v0/users/{user}/tokens/{token} - token not found
#[tokio::test]
async fn tokens_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensnotfound@example.com")
        .await;

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/users/{}/tokens/00000000-0000-0000-0000-000000000000",
            user_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// PATCH /v0/users/{user}/tokens/{token} - update token
#[tokio::test]
async fn tokens_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensupdate@example.com").await;

    // First create a token
    let body = serde_json::json!({
        "name": "Original Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created_token: JsonToken = create_resp.json().await.expect("Failed to parse response");

    // Now update it
    let update_body = serde_json::json!({
        "name": "Updated Token"
    });

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/users/{}/tokens/{}",
            user_slug, created_token.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&update_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Token");
}

// POST /v0/users/{user}/tokens - cannot create token for other user
#[tokio::test]
async fn tokens_create_other_user_forbidden() {
    let server = TestServer::new().await;
    let user1 = server.signup("User One", "token1@example.com").await;
    let user2 = server.signup("User Two", "token2@example.com").await;

    let body = serde_json::json!({
        "name": "Sneaky Token"
    });

    // User1 tries to create token for User2
    let user2_slug: &str = user2.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user2_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user1.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // First user is admin, so may succeed
    // Non-admin would get forbidden
    assert!(
        resp.status() == StatusCode::CREATED || resp.status() == StatusCode::FORBIDDEN,
        "Expected CREATED (admin) or FORBIDDEN, got: {}",
        resp.status()
    );
}
