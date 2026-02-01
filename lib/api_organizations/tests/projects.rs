#![allow(unused_crate_dependencies, clippy::tests_outside_test_module, clippy::redundant_test_prefix, clippy::uninlined_format_args)]
//! Integration tests for organization projects endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonProject, JsonProjects};
use http::StatusCode;

// GET /v0/organizations/{organization}/projects - list projects
#[tokio::test]
async fn test_projects_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projlist@example.com").await;
    let org = server.create_org(&user, "Projects Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let projects: JsonProjects = resp.json().await.expect("Failed to parse response");
    // New org should have no projects
    assert!(projects.0.is_empty());
}

// GET /v0/organizations/{organization}/projects - list with projects
#[tokio::test]
async fn test_projects_list_with_project() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projlistwith@example.com").await;
    let org = server.create_org(&user, "Projects With Org").await;
    let _project = server.create_project(&user, &org, "Test Project").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let projects: JsonProjects = resp.json().await.expect("Failed to parse response");
    assert_eq!(projects.0.len(), 1);
}

// POST /v0/organizations/{organization}/projects - create project
#[tokio::test]
async fn test_projects_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projcreate@example.com").await;
    let org = server.create_org(&user, "Create Project Org").await;

    let body = serde_json::json!({
        "name": "New Project",
        "slug": "new-project"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let project: JsonProject = resp.json().await.expect("Failed to parse response");
    assert_eq!(project.name.as_ref(), "New Project");
}

// POST /v0/organizations/{organization}/projects - create with URL
#[tokio::test]
async fn test_projects_create_with_url() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projurl@example.com").await;
    let org = server.create_org(&user, "URL Project Org").await;

    let body = serde_json::json!({
        "name": "URL Project",
        "url": "https://github.com/example/project"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// POST /v0/organizations/{organization}/projects - duplicate slug fails
#[tokio::test]
async fn test_projects_create_duplicate_slug() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projdup@example.com").await;
    let org = server.create_org(&user, "Dup Project Org").await;

    let body = serde_json::json!({
        "name": "Dup Project",
        "slug": "dup-project"
    });

    let org_slug: &str = org.slug.as_ref();

    // First project
    let resp = server
        .client
        .post(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second project with same slug should fail
    let resp = server
        .client
        .post(server.api_url(&format!(
            "/v0/organizations/{}/projects",
            org_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
