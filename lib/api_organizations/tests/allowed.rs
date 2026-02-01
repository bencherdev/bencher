//! Integration tests for organization permission endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonAllowed;
use http::StatusCode;

// GET /v0/organizations/{organization}/allowed/{permission} - view permission
#[tokio::test]
async fn test_allowed_view() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "allowedview@example.com").await;
    let org = server.create_org(&user, "Allowed Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/allowed/view",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let allowed: JsonAllowed = resp.json().await.expect("Failed to parse response");
    assert!(allowed.allowed);
}

// GET /v0/organizations/{organization}/allowed/{permission} - edit permission
#[tokio::test]
async fn test_allowed_edit() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "allowededit@example.com").await;
    let org = server.create_org(&user, "Edit Allowed Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/allowed/edit",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let allowed: JsonAllowed = resp.json().await.expect("Failed to parse response");
    // Org creator should have edit permission
    assert!(allowed.allowed);
}

// GET /v0/organizations/{organization}/allowed/{permission} - delete permission
#[tokio::test]
async fn test_allowed_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alloweddelete@example.com").await;
    let org = server.create_org(&user, "Delete Allowed Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/allowed/delete",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let allowed: JsonAllowed = resp.json().await.expect("Failed to parse response");
    // Org creator should have delete permission
    assert!(allowed.allowed);
}

// GET /v0/organizations/{organization}/allowed/{permission} - invalid permission
#[tokio::test]
async fn test_allowed_invalid_permission() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "allowedinvalid@example.com").await;
    let org = server.create_org(&user, "Invalid Allowed Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/allowed/invalid_permission",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Invalid permission should return bad request
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
