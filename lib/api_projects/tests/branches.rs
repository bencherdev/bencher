#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project branch endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonBranch, JsonBranches};
use http::StatusCode;

// GET /v0/projects/{project}/branches - list branches
#[tokio::test]
async fn branches_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchlist@example.com").await;
    let org = server.create_org(&user, "Branch Org").await;
    let project = server.create_project(&user, &org, "Branch Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _branches: JsonBranches = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/branches - create branch
#[tokio::test]
async fn branches_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchcreate@example.com").await;
    let org = server.create_org(&user, "Branch Create Org").await;
    let project = server
        .create_project(&user, &org, "Branch Create Project")
        .await;

    let body = serde_json::json!({
        "name": "feature-branch",
        "slug": "feature-branch"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse response");
    assert_eq!(branch.name.as_ref(), "feature-branch");
}

// POST /v0/projects/{project}/branches - auto-generate slug
#[tokio::test]
async fn branches_create_auto_slug() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "branchautoslug@example.com")
        .await;
    let org = server.create_org(&user, "Branch Auto Org").await;
    let project = server
        .create_project(&user, &org, "Branch Auto Project")
        .await;

    // Branch names follow git naming rules - no spaces allowed
    let body = serde_json::json!({
        "name": "auto-slug-branch"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// GET /v0/projects/{project}/branches/{branch} - get branch
#[tokio::test]
async fn branches_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchget@example.com").await;
    let org = server.create_org(&user, "Branch Get Org").await;
    let project = server
        .create_project(&user, &org, "Branch Get Project")
        .await;

    // Create a branch first
    let body = serde_json::json!({
        "name": "get-branch",
        "slug": "get-branch"
    });

    let project_slug: &str = project.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created: JsonBranch = create_resp.json().await.expect("Failed to parse response");

    // Get the branch
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/{}",
            project_slug, created.slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// DELETE /v0/projects/{project}/branches/{branch} - delete branch
#[tokio::test]
async fn branches_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchdelete@example.com").await;
    let org = server.create_org(&user, "Branch Delete Org").await;
    let project = server
        .create_project(&user, &org, "Branch Delete Project")
        .await;

    // Create a branch first
    let body = serde_json::json!({
        "name": "delete-branch",
        "slug": "delete-branch"
    });

    let project_slug: &str = project.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // Delete the branch
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/branches/delete-branch",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
