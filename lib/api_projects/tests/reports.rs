#![allow(unused_crate_dependencies, clippy::tests_outside_test_module, clippy::redundant_test_prefix, clippy::uninlined_format_args)]
//! Integration tests for project report endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonReports;
use http::StatusCode;

// GET /v0/projects/{project}/reports - list reports (empty)
#[tokio::test]
async fn test_reports_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportlist@example.com").await;
    let org = server.create_org(&user, "Report Org").await;
    let project = server.create_project(&user, &org, "Report Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    // New project should have no reports
    assert!(reports.0.is_empty());
}

// GET /v0/projects/{project}/reports - with pagination
#[tokio::test]
async fn test_reports_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportpage@example.com").await;
    let org = server.create_org(&user, "Report Page Org").await;
    let project = server.create_project(&user, &org, "Report Page Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports?per_page=10&page=1",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/reports/{report} - not found
#[tokio::test]
async fn test_reports_get_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportnotfound@example.com").await;
    let org = server.create_org(&user, "Report NotFound Org").await;
    let project = server.create_project(&user, &org, "Report NotFound Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// DELETE /v0/projects/{project}/reports/{report} - not found
#[tokio::test]
async fn test_reports_delete_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportdelnotfound@example.com").await;
    let org = server.create_org(&user, "Report Del NotFound Org").await;
    let project = server.create_project(&user, &org, "Report Del NotFound Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/reports/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
