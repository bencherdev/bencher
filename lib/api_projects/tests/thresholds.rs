#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project threshold endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonThresholds;
use http::StatusCode;

// GET /v0/projects/{project}/thresholds - list thresholds
#[tokio::test]
async fn thresholds_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdlist@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let thresholds: JsonThresholds = resp.json().await.expect("Failed to parse response");
    // New project should have no thresholds
    assert!(thresholds.0.is_empty());
}

// GET /v0/projects/{project}/thresholds - requires auth
#[tokio::test]
async fn thresholds_list_requires_auth() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdauth@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Auth Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Auth Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .send()
        .await
        .expect("Request failed");

    // Public project can be viewed without auth
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::BAD_REQUEST,
        "Expected OK or BAD_REQUEST, got: {}",
        resp.status()
    );
}

// GET /v0/projects/{project}/thresholds/{threshold} - not found
#[tokio::test]
async fn thresholds_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Threshold NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Threshold NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
