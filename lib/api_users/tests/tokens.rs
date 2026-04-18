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

// DELETE /v0/users/{user}/tokens/{token} - revoke succeeds and hides from default list
#[tokio::test]
async fn tokens_revoke_hides_from_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevokelist@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let body = serde_json::json!({ "name": "Revoke Me" });
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
    let created: JsonToken = create_resp.json().await.expect("Failed to parse response");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
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
        tokens.0.iter().all(|t| t.uuid != created.uuid),
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
        .find(|t| t.uuid == created.uuid)
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

    let body = serde_json::json!({ "name": "Visible After Revoke" });
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
    let created: JsonToken = create_resp.json().await.expect("Failed to parse response");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
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
        .get(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
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

    let body = serde_json::json!({ "name": "About To Be Revoked" });
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
    let created: JsonToken = create_resp.json().await.expect("Failed to parse response");

    // The freshly minted JWT authenticates.
    let pre_auth = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&created.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(pre_auth.status(), StatusCode::OK);

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
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
            bencher_json::bearer_header(&created.token),
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

    let body = serde_json::json!({ "name": "Owner Token" });
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", owner_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&owner.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created: JsonToken = create_resp.json().await.expect("Failed to parse response");

    let revoke_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", owner_slug, created.uuid)))
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

// DELETE /v0/users/{user}/tokens/{token} - revoking the same token twice is a 4xx
#[tokio::test]
async fn tokens_revoke_double_rejected() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "tokensrevoketwice@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();

    let body = serde_json::json!({ "name": "Revoke Twice" });
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
    let created: JsonToken = create_resp.json().await.expect("Failed to parse response");

    let first = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
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
        .delete(server.api_url(&format!("/v0/users/{}/tokens/{}", user_slug, created.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert!(
        second.status().is_client_error(),
        "second revocation must return a 4xx, got: {}",
        second.status()
    );
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
