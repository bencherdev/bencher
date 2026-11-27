#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for user token endpoints.
//!
//! User API tokens are deprecated: none can be made, and an existing token can
//! only be listed and revoked, so these tests seed tokens directly into the
//! database via `seed_token`.

use bencher_api_tests::{TestServer, helpers::seed_token};
use bencher_json::JsonTokens;
use http::{Method, StatusCode};

// A token route's preflight offers only the method the route still has: list
// on the tokens, and revoke on a token.
// Kills a preflight that still offers creating, viewing, or updating a token.
#[cfg(feature = "plus")]
#[tokio::test]
async fn tokens_preflights_offer_list_and_revoke_only() {
    let server = TestServer::new_with_options(bencher_api_tests::TestServerOptions {
        http_options: true,
        ..bencher_api_tests::TestServerOptions::default()
    })
    .await;
    let user = server
        .signup("Test User", "tokenspreflight@example.com")
        .await;
    let seeded = seed_token(&server, &user, "Preflight Token");
    let user_slug: &str = user.slug.as_ref();
    let tokens = format!("/v0/users/{}/tokens", user_slug);
    let token = format!("{}/{}", tokens, seeded.uuid);

    for (path, methods) in [(&tokens, "GET"), (&token, "DELETE")] {
        let resp = server
            .client
            .request(Method::OPTIONS, server.api_url(path))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK, "{}", path);
        let allowed = resp
            .headers()
            .get("access-control-allow-methods")
            .and_then(|value| value.to_str().ok());
        assert_eq!(allowed, Some(methods), "{}", path);
    }
}

// Creating, viewing, and updating a token are gone: each answers what the
// router answers a method its path no longer has, and changes nothing.
// Kills a create, view, or update route left in place.
#[tokio::test]
async fn tokens_create_view_and_update_are_gone() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokensgone@example.com").await;
    let seeded = seed_token(&server, &user, "Kept Token");
    let user_slug: &str = user.slug.as_ref();
    let tokens = format!("/v0/users/{}/tokens", user_slug);
    let token = format!("{}/{}", tokens, seeded.uuid);

    for (method, path) in [
        (Method::POST, &tokens),
        (Method::GET, &token),
        (Method::PATCH, &token),
    ] {
        let resp = server
            .client
            .request(method.clone(), server.api_url(path))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({"name": "Changed Token"}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{} {}",
            method,
            path
        );
    }

    let resp = server
        .client
        .get(server.api_url(&tokens))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let listed: JsonTokens = resp.json().await.expect("Failed to parse response");
    let names: Vec<String> = listed.0.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(names, ["Kept Token"]);
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

// GET /v0/users/{user}/tokens?sort=created - oldest first, a page at a time
#[tokio::test]
async fn tokens_list_sort_created() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "tokenssort@example.com").await;
    for name in ["charlie", "alpha", "bravo"] {
        seed_token(&server, &user, name);
    }

    let user_slug: &str = user.slug.as_ref();
    let names = |query: &'static str| {
        let server = &server;
        let token = &user.token;
        async move {
            let resp = server
                .client
                .get(server.api_url(&format!("/v0/users/{}/tokens?{}", user_slug, query)))
                .header(
                    bencher_json::AUTHORIZATION,
                    bencher_json::bearer_header(token),
                )
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), StatusCode::OK);
            let tokens: JsonTokens = resp.json().await.expect("Failed to parse response");
            tokens
                .0
                .into_iter()
                .map(|token| token.name.to_string())
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(names("per_page=2").await, ["alpha", "bravo"]);
    assert_eq!(names("sort=created&per_page=2").await, ["charlie", "alpha"]);
    assert_eq!(names("sort=created&per_page=2&page=2").await, ["bravo"]);
    assert_eq!(
        names("sort=created&direction=desc").await,
        ["bravo", "alpha", "charlie"]
    );
}
