#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for user token endpoints.
//!
//! User API tokens are deprecated: `POST /v0/users/{user}/tokens` always fails
//! with a `403 Forbidden` error pointing at user API keys. Existing tokens can
//! still be listed, viewed, updated, and revoked, so those tests seed tokens
//! directly into the database via `seed_token`.

use bencher_api_tests::{TestServer, helpers::seed_token};
use bencher_json::{JsonToken, JsonTokens};
use http::StatusCode;

// POST /v0/users/{user}/tokens - token creation is deprecated and always fails
#[tokio::test]
async fn tokens_create_deprecated() {
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

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let message = resp.text().await.expect("Failed to read response");
    assert!(
        message.contains("deprecated"),
        "the 403 must explain the deprecation: {message}"
    );

    // No token was created
    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let tokens: JsonTokens = list_resp.json().await.expect("Failed to parse response");
    assert!(tokens.0.is_empty());
}

// POST /v0/users/{user}/tokens - invalid credentials still get a 401, not the 403
#[tokio::test]
async fn tokens_create_unauthenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensnoauth@example.com").await;

    let body = serde_json::json!({
        "name": "No Auth Token"
    });

    // Valid format, no row in `user_key` table.
    let unknown = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(unknown),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/users/{user}/tokens - deprecated even for another user's path
#[tokio::test]
async fn tokens_create_other_user_deprecated() {
    let server = TestServer::new().await;
    let user1 = server.signup("User One", "token1@example.com").await;
    let user2 = server.signup("User Two", "token2@example.com").await;

    let body = serde_json::json!({
        "name": "Sneaky Token"
    });

    // User1 tries to create a token for User2
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

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// GET /v0/users/{user}/tokens - list tokens (after seeding one)
#[tokio::test]
async fn tokens_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokenslist@example.com").await;

    let seeded = seed_token(&server, &user, "List Test Token");

    let user_slug: &str = user.slug.as_ref();
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
    assert!(tokens.0.iter().any(|t| t.uuid == seeded.uuid));
}

// GET /v0/users/{user}/tokens/{token} - view token
#[tokio::test]
async fn tokens_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensget@example.com").await;

    let seeded = seed_token(&server, &user, "View Token");

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonToken = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, seeded.uuid);
    assert_eq!(fetched.name.as_ref(), "View Token");
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

    let seeded = seed_token(&server, &user, "Original Token");

    let update_body = serde_json::json!({
        "name": "Updated Token"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
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

// DELETE /v0/users/{user}/tokens/{token} - revoke succeeds and hides from default list
#[tokio::test]
async fn tokens_revoke_hides_from_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevokelist@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let seeded = seed_token(&server, &user, "Revoke Me");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(revoke_resp.status(), StatusCode::NO_CONTENT);

    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let tokens: JsonTokens = list_resp.json().await.expect("Failed to parse response");
    assert!(
        tokens.0.iter().all(|t| t.uuid != seeded.uuid),
        "revoked token must be hidden from default list: {:?}",
        tokens
    );

    let list_with_revoked = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens?revoked=true", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_with_revoked.status(), StatusCode::OK);
    let tokens: JsonTokens = list_with_revoked
        .json()
        .await
        .expect("Failed to parse response");
    let entry = tokens
        .0
        .iter()
        .find(|t| t.uuid == seeded.uuid)
        .expect("revoked token should appear when `?revoked=true`");
    assert!(
        entry.revoked.is_some(),
        "revoked entry must carry a revoked timestamp: {:?}",
        entry
    );
}

// DELETE /v0/users/{user}/tokens/{token} - revoked token still readable via direct GET
#[tokio::test]
async fn tokens_revoke_visible_on_direct_get() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevokeget@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let seeded = seed_token(&server, &user, "Visible After Revoke");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(revoke_resp.status(), StatusCode::NO_CONTENT);

    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::OK);
    let fetched: JsonToken = get_resp.json().await.expect("Failed to parse response");
    assert!(
        fetched.revoked.is_some(),
        "direct GET on revoked token must expose the revoked timestamp: {:?}",
        fetched
    );
}

// DELETE /v0/users/{user}/tokens/{token} - revoked JWT stops authenticating
#[tokio::test]
async fn tokens_revoke_breaks_auth() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevokeauth@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let seeded = seed_token(&server, &user, "About To Be Revoked");

    // The seeded JWT authenticates.
    let pre_auth = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&seeded.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(pre_auth.status(), StatusCode::OK);

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(revoke_resp.status(), StatusCode::NO_CONTENT);

    // Same JWT, now rejected as 401.
    let post_auth = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&seeded.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(post_auth.status(), StatusCode::UNAUTHORIZED);
}

// DELETE /v0/users/{user}/tokens/{token} - another non-admin user cannot revoke
#[tokio::test]
async fn tokens_revoke_other_user_forbidden() {
    let server = TestServer::new().await;
    let owner = server.signup("Owner", "revokeowner@example.com").await;
    // First signup is admin on this server; ensure the attacker is not the owner.
    let attacker = server
        .signup("Attacker", "revokeattacker@example.com")
        .await;
    let owner_slug: &str = owner.slug.as_ref();

    let seeded = seed_token(&server, &owner, "Owner Token");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", owner_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&attacker.token),
        )
        .send()
        .await
        .expect("Request failed");
    // `same_user!` returns 4xx when a non-admin user acts on another user's resource.
    assert!(
        revoke_resp.status().is_client_error(),
        "non-admin attacker must not be able to revoke another user's token, got: {}",
        revoke_resp.status()
    );
}

// DELETE /v0/users/{user}/tokens/{token} - admin can revoke another user's token
#[tokio::test]
async fn tokens_revoke_admin_can_revoke_other_user() {
    let server = TestServer::new().await;
    // First signup is admin on this server.
    let admin = server.signup("Admin", "revokeadmin@example.com").await;
    let target = server.signup("Target", "revoketarget@example.com").await;
    let target_slug: &str = target.slug.as_ref();

    let seeded = seed_token(&server, &target, "Target Token");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", target_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        revoke_resp.status(),
        StatusCode::NO_CONTENT,
        "admin must be able to revoke another user's token"
    );
}

// DELETE /v0/users/{user}/tokens/{token} - revoking the same token twice is a 4xx
#[tokio::test]
async fn tokens_revoke_double_rejected() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevoketwice@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let seeded = seed_token(&server, &user, "Revoke Twice");

    let first = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(first.status(), StatusCode::NO_CONTENT);

    let second = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, seeded.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(second.status(), StatusCode::CONFLICT);
}
