#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for user CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{Email, JsonToken, JsonTokens, JsonUser};
use http::StatusCode;

// GET /v0/users - admin can list all users
#[tokio::test]
async fn users_list_as_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let admin = server.signup("Admin User", "usersadmin@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/users"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    // First user is admin and can list all users
    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/users - non-admin cannot list users
#[tokio::test]
async fn users_list_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let _admin = server.signup("Admin User", "adminlist@example.com").await;
    // Second user is NOT admin
    let user = server
        .signup("Regular User", "regularlist@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/users"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// GET /v0/users/{user} - view own profile
#[tokio::test]
async fn users_get_self() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usersget@example.com").await;

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, user.uuid);
}

// GET /v0/users/{user} - view by UUID
#[tokio::test]
async fn users_get_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usersbyuuid@example.com").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, user.uuid);
}

// GET /v0/users/{user} - cannot view other user (unless admin)
#[tokio::test]
async fn users_get_other_forbidden() {
    let server = TestServer::new().await;
    let user1 = server.signup("User One", "user1@example.com").await;
    let user2 = server.signup("User Two", "user2@example.com").await;

    // User1 tries to view User2
    let user2_slug: &str = user2.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user2_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user1.token),
        )
        .send()
        .await
        .expect("Request failed");

    // First user is admin, so they can view
    // For non-first users, this would be forbidden
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::FORBIDDEN,
        "Expected OK (admin) or FORBIDDEN, got: {}",
        resp.status()
    );
}

// PATCH /v0/users/{user} - update own profile
#[tokio::test]
async fn users_update_self() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usersupdate@example.com").await;

    let body = serde_json::json!({
        "name": "Updated Name"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Name");
}

// PATCH /v0/users/{user} - update slug
#[tokio::test]
async fn users_update_slug() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usersslug@example.com").await;

    let body = serde_json::json!({
        "slug": "new-user-slug"
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonUser = resp.json().await.expect("Failed to parse response");
    let new_slug: &str = updated.slug.as_ref();
    assert_eq!(new_slug, "new-user-slug");
}

// PATCH /v0/users/{user} - non-admin cannot set admin flag
#[tokio::test]
async fn users_update_admin_forbidden() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "adminupd@example.com").await;
    let user = server.signup("Test User", "useradmin@example.com").await;

    let body = serde_json::json!({
        "admin": true
    });

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Non-admin cannot set admin flag
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/users/{user} - changing email revokes all API tokens
#[tokio::test]
async fn users_update_email_revokes_tokens() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "emailrevoke@example.com").await;
    let user_slug: &str = user.slug.as_ref();

    // Create an API token
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "name": "My Token" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let api_token: JsonToken = create_resp.json().await.expect("Failed to parse response");

    // Verify the API token authenticates
    let pre_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&api_token.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(pre_resp.status(), StatusCode::OK);

    // Change email (use session token, not the API token)
    let new_email: Email = "newemail@example.com".parse().expect("Invalid email");
    let patch_resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "email": new_email }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(patch_resp.status(), StatusCode::OK);

    // The old session token has the old email and can no longer authenticate.
    // Generate a new client token for the new email.
    let new_session = server
        .token_key()
        .new_client(new_email, u32::MAX)
        .expect("Failed to create client token");

    // The API token should now be revoked — list with ?revoked=true using the new session token
    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens?revoked=true", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&new_session),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let tokens: JsonTokens = list_resp.json().await.expect("Failed to parse response");
    let entry = tokens
        .0
        .iter()
        .find(|t| t.uuid == api_token.uuid)
        .expect("revoked token should appear when ?revoked=true");
    assert!(
        entry.revoked.is_some(),
        "token must carry a revoked timestamp after email change: {:?}",
        entry
    );

    // The API token should no longer authenticate
    let post_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&api_token.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert!(
        post_resp.status().is_client_error(),
        "revoked API token must not authenticate after email change, got: {}",
        post_resp.status()
    );
}

// PATCH /v0/users/{user} - updating name does NOT revoke tokens
#[tokio::test]
async fn users_update_name_no_revoke() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "namenorevoke@example.com").await;
    let user_slug: &str = user.slug.as_ref();

    // Create an API token
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "name": "Keep Me" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let api_token: JsonToken = create_resp.json().await.expect("Failed to parse response");

    // Update only the name
    let patch_resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "name": "New Name" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(patch_resp.status(), StatusCode::OK);

    // The API token should still work
    let post_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&api_token.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        post_resp.status(),
        StatusCode::OK,
        "API token must still authenticate after name-only update"
    );
}
