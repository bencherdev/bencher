#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonProject, JsonProjects};
use http::StatusCode;

// GET /v0/projects - list all public projects
#[tokio::test]
async fn projects_list_public() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _projects: JsonProjects = resp.json().await.expect("Failed to parse response");
}

// GET /v0/projects - list with auth header
#[tokio::test]
async fn projects_list_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projlistauth@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project} - get a project
#[tokio::test]
async fn projects_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projget@example.com").await;
    let org = server.create_org(&user, "Project Org").await;
    let project = server.create_project(&user, &org, "Test Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonProject = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, project.uuid);
}

// GET /v0/projects/{project} - by UUID
#[tokio::test]
async fn projects_get_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projuuid@example.com").await;
    let org = server.create_org(&user, "UUID Org").await;
    let project = server.create_project(&user, &org, "UUID Project").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project} - not found
#[tokio::test]
async fn projects_get_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projnotfound@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects/nonexistent-project"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// PATCH /v0/projects/{project} - update a project
#[tokio::test]
async fn projects_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projupdate@example.com").await;
    let org = server.create_org(&user, "Update Org").await;
    let project = server.create_project(&user, &org, "Update Project").await;

    let body = serde_json::json!({
        "name": "Updated Project Name"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonProject = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Project Name");
}

// PATCH /v0/projects/{project} - update URL
#[tokio::test]
async fn projects_update_url() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projurlupd@example.com").await;
    let org = server.create_org(&user, "URL Update Org").await;
    let project = server
        .create_project(&user, &org, "URL Update Project")
        .await;

    let body = serde_json::json!({
        "url": "https://github.com/updated/project"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// DELETE /v0/projects/{project} - delete a project
#[tokio::test]
async fn projects_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projdelete@example.com").await;
    let org = server.create_org(&user, "Delete Org").await;
    let project = server.create_project(&user, &org, "Delete Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify project is deleted
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}
