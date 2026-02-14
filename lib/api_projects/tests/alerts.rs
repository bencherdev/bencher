#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project alert endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonAlerts;
use http::StatusCode;

// GET /v0/projects/{project}/alerts - list alerts (empty)
#[tokio::test]
async fn alerts_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alertlist@example.com").await;
    let org = server.create_org(&user, "Alert Org").await;
    let project = server.create_project(&user, &org, "Alert Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/alerts", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let alerts: JsonAlerts = resp.json().await.expect("Failed to parse response");
    // New project should have no alerts
    assert!(alerts.0.is_empty());
}

// GET /v0/projects/{project}/alerts - with pagination
#[tokio::test]
async fn alerts_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alertpage@example.com").await;
    let org = server.create_org(&user, "Alert Page Org").await;
    let project = server
        .create_project(&user, &org, "Alert Page Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts?per_page=10&page=1",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/alerts/{alert} - not found
#[tokio::test]
async fn alerts_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "alertnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Alert NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Alert NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
