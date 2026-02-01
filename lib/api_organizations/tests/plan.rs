//! Integration tests for organization plan endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// GET /v0/organizations/{organization}/plan - view plan
#[tokio::test]
async fn test_plan_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "planget@example.com").await;
    let org = server.create_org(&user, "Plan Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/plan",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Plan may or may not exist - both OK and NOT_FOUND are valid
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/plan - requires auth
#[tokio::test]
async fn test_plan_requires_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "planauth@example.com").await;
    let org = server.create_org(&user, "Plan Auth Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/plan",
            org_slug
        )))
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

// GET /v0/organizations/{organization}/plan - not found org
#[tokio::test]
async fn test_plan_org_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "plannotfound@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations/nonexistent-org/plan"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
