#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for organization usage endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// GET /v0/organizations/{organization}/usage - view usage
#[tokio::test]
async fn test_usage_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageget@example.com").await;
    let org = server.create_org(&user, "Usage Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/usage", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Usage endpoint may return NOT_FOUND if no usage data exists yet
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/usage - requires auth
#[tokio::test]
async fn test_usage_requires_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageauth@example.com").await;
    let org = server.create_org(&user, "Usage Auth Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/usage", org_slug)))
        .send()
        .await
        .expect("Request failed");

    // Without auth, should get an error (either 400 Bad Request or 404 Not Found)
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "Expected 400 or 404, got: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/usage - not found org
#[tokio::test]
async fn test_usage_org_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "usagenotfound@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations/nonexistent-org/usage"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
