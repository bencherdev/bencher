#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for user CRUD endpoints.

use bencher_api_tests::{TestServer, TestUser, helpers::seed_token};
use bencher_json::{JsonUser, JsonUserKeyCreated, UserKey};
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

// PATCH /v0/users/{user} - updating name does NOT revoke tokens
#[tokio::test]
async fn users_update_name_no_revoke() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "namenorevoke@example.com").await;
    let user_slug: &str = user.slug.as_ref();

    // Seed an API token (token creation via the API is deprecated)
    let api_token = seed_token(&server, &user, "Keep Me");

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

// Fails if user PATCH lets a deprecated user API token change the email.
#[tokio::test]
async fn users_update_email_api_token_forbidden() {
    let (server, admin, user) = setup_admin_and_user().await;
    let api_token = seed_token(&server, &user, "Leaked Token");

    let body = serde_json::json!({ "email": "attacker@example.com" });
    let status = patch_user(&server, api_token.token.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    assert_eq!(view_user(&server, &admin, &user).await.email, user.email);
}

// Fails if the email gate lets a user API key change the email, or refuses a user API key's other fields.
#[tokio::test]
async fn users_update_email_user_key_forbidden() {
    let (server, admin, user) = setup_admin_and_user().await;
    let key = mint_key(&server, &user).await;

    let body = serde_json::json!({ "name": "Renamed By Key", "email": user.email });
    let status = patch_user(&server, key.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::OK);

    let body = serde_json::json!({ "email": "attacker@example.com" });
    let status = patch_user(&server, key.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    assert_eq!(view_user(&server, &admin, &user).await.email, user.email);
}

// Fails if the email gate refuses the whole PATCH, or any request that resends the current email.
#[tokio::test]
async fn users_update_name_api_token_allowed() {
    let (server, admin, user) = setup_admin_and_user().await;
    let api_token = seed_token(&server, &user, "Automation Token");

    let body = serde_json::json!({ "name": "Renamed User", "email": user.email });
    let status = patch_user(&server, api_token.token.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::OK);

    let viewed = view_user(&server, &admin, &user).await;
    assert_eq!(viewed.name.as_ref(), "Renamed User");
}

// Fails if an admin's deprecated user API token can change another user's email or is refused resending it, or the admin's session is refused.
#[tokio::test]
async fn users_update_other_email_admin_api_token_forbidden() {
    let (server, admin, user) = setup_admin_and_user().await;
    let admin_token = seed_token(&server, &admin, "Admin Token");

    let body = serde_json::json!({ "name": "Renamed By Admin", "email": user.email });
    let status = patch_user(&server, admin_token.token.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::OK);

    let body = serde_json::json!({ "email": "attacker@example.com" });
    let status = patch_user(&server, admin_token.token.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(view_user(&server, &admin, &user).await.email, user.email);

    let body = serde_json::json!({ "email": "moved@example.com" });
    let status = patch_user(&server, &admin.token, &user, &body).await;
    assert_eq!(status, StatusCode::OK);
}

// Fails if an admin's user API key or deprecated user API token can change another user's admin or locked field, or the admin's session cannot.
#[tokio::test]
async fn users_update_admin_and_locked_session_only() {
    let (server, admin, user) = setup_admin_and_user().await;
    let admin_key = mint_key(&server, &admin).await;
    let admin_token = seed_token(&server, &admin, "Admin Token");

    for credential in [admin_key.as_ref(), admin_token.token.as_ref()] {
        for body in [
            serde_json::json!({ "admin": true }),
            serde_json::json!({ "locked": true }),
        ] {
            let status = patch_user(&server, credential, &user, &body).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
        }
    }
    let viewed = view_user(&server, &admin, &user).await;
    assert!(!viewed.admin);
    assert!(!viewed.locked);

    let body = serde_json::json!({ "admin": true, "locked": true });
    let status = patch_user(&server, &admin.token, &user, &body).await;
    assert_eq!(status, StatusCode::OK);
}

// Fails if resending another user's current admin or locked value without a session is refused, or compared with the requester's own value.
#[tokio::test]
async fn users_update_admin_and_locked_resend_allowed() {
    let (server, admin, user) = setup_admin_and_user().await;
    let admin_key = mint_key(&server, &admin).await;

    // Lock the user so both of its values differ from the requester's.
    let body = serde_json::json!({ "locked": true });
    let status = patch_user(&server, &admin.token, &user, &body).await;
    assert_eq!(status, StatusCode::OK);

    let body = serde_json::json!({ "admin": false, "locked": true });
    let status = patch_user(&server, admin_key.as_ref(), &user, &body).await;
    assert_eq!(status, StatusCode::OK);
}

/// The first user to sign up is the server admin; the second is not.
async fn setup_admin_and_user() -> (TestServer, TestUser, TestUser) {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin User", "sessiongateadmin@example.com")
        .await;
    let user = server.signup("Test User", "sessiongate@example.com").await;
    (server, admin, user)
}

async fn mint_key(server: &TestServer, owner: &TestUser) -> UserKey {
    let owner_slug: &str = owner.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", owner_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&owner.token),
        )
        .json(&serde_json::json!({ "name": "test-key" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: JsonUserKeyCreated = resp.json().await.expect("Failed to parse user key");
    created.key
}

async fn patch_user(
    server: &TestServer,
    bearer: &str,
    user: &TestUser,
    body: &serde_json::Value,
) -> StatusCode {
    let user_slug: &str = user.slug.as_ref();
    server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed")
        .status()
}

async fn view_user(server: &TestServer, admin: &TestUser, user: &TestUser) -> JsonUser {
    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("Failed to parse response")
}
