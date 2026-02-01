//! Integration tests for user endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonToken, JsonTokens, JsonUser};
use http::StatusCode;

// GET /v0/users - requires admin (first user is admin)
#[tokio::test]
async fn test_users_list_as_admin() {
    let server = TestServer::new().await;
    // First user created in a fresh DB is admin
    let admin = server.signup("Admin User", "admin@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/users"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    // First user is admin and can list all users
    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/users/{user} - view own profile
#[tokio::test]
async fn test_users_get_self() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "userget@example.com").await;

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, user.uuid);
}

// PATCH /v0/users/{user} - update profile
#[tokio::test]
async fn test_users_update_self() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "userupdate@example.com").await;

    let body = serde_json::json!({
        "name": "Updated Name"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Name");
}

// GET /v0/users/{user}/tokens - list tokens
#[tokio::test]
async fn test_users_tokens_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usertokens@example.com").await;

    // First create a token so the list isn't empty
    let create_body = serde_json::json!({
        "name": "List Test Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&create_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // Now list tokens
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let tokens: JsonTokens = resp.json().await.expect("Failed to parse response");
    // User should have the token we just created
    assert!(!tokens.0.is_empty());
}

// POST /v0/users/{user}/tokens - create token
#[tokio::test]
async fn test_users_tokens_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "createtoken@example.com").await;

    let body = serde_json::json!({
        "name": "Test Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let token: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(token.name.as_ref(), "Test Token");
}

// GET /v0/users/{user}/tokens/{token} - view token
#[tokio::test]
async fn test_users_tokens_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "gettoken@example.com").await;

    // First create a token
    let body = serde_json::json!({
        "name": "View Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, created_token.uuid);
}

// PATCH /v0/users/{user}/tokens/{token} - update token
#[tokio::test]
async fn test_users_tokens_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "updatetoken@example.com").await;

    // First create a token
    let body = serde_json::json!({
        "name": "Original Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
        .json(&update_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Token");
}
