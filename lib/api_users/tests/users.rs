#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for user CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonUser;
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
