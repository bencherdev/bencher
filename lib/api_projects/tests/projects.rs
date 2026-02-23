#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonNewProject, JsonProject, JsonProjects, ProjectUuid};
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

// Soft-delete removes project from list
#[tokio::test]
async fn projects_soft_delete_not_in_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projsoftdel@example.com").await;
    let org = server.create_org(&user, "Soft Delete Org").await;
    let project = server
        .create_project(&user, &org, "Soft Delete Project")
        .await;

    // Soft-delete (default)
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify absent from list
    let list_resp = server
        .client
        .get(server.api_url("/v0/projects"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let projects: JsonProjects = list_resp.json().await.expect("Failed to parse response");
    assert!(
        !projects.0.iter().any(|p| p.uuid == project.uuid),
        "Soft-deleted project should not appear in list"
    );
}

// Soft-delete frees slug for reuse
#[tokio::test]
async fn projects_soft_delete_slug_reuse() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "projslugresue@example.com")
        .await;
    let org = server.create_org(&user, "Slug Reuse Org").await;
    let project = server
        .create_project(&user, &org, "Slug Reuse Project")
        .await;

    // Soft-delete
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Create new project with the same slug
    let body = JsonNewProject {
        name: "Slug Reuse Project".parse().unwrap(),
        slug: Some(project.slug.clone()),
        url: None,
        visibility: None,
    };
    let org_slug: &str = org.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{org_slug}/projects")))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
}

// Hard delete requires server admin
#[tokio::test]
async fn projects_hard_delete_requires_admin() {
    let server = TestServer::new().await;
    // First signup is admin
    let _admin = server.signup("Admin", "projhardadm@example.com").await;
    // Second signup is NOT admin
    let user = server
        .signup("Regular User", "projharduser@example.com")
        .await;
    let org = server.create_org(&user, "Hard Delete Org").await;
    let project = server
        .create_project(&user, &org, "Hard Delete Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}?hard=true")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// Admin can hard-delete
#[tokio::test]
async fn projects_hard_delete_as_admin() {
    let server = TestServer::new().await;
    // First signup is admin
    let admin = server.signup("Admin User", "projhardok@example.com").await;
    let org = server.create_org(&admin, "Admin Hard Del Org").await;
    let project = server
        .create_project(&admin, &org, "Admin Hard Del Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}?hard=true")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify truly gone
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// GET by UUID returns 404 after soft-delete
#[tokio::test]
async fn projects_soft_delete_get_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projsduuid@example.com").await;
    let org = server.create_org(&user, "UUID Del Org").await;
    let project = server.create_project(&user, &org, "UUID Del Project").await;

    // Soft-delete
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET by UUID should return 404
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// Second soft-delete returns 404 (idempotent)
#[tokio::test]
async fn projects_soft_delete_idempotent() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "projsdidempotent@example.com")
        .await;
    let org = server.create_org(&user, "Idempotent Del Org").await;
    let project = server
        .create_project(&user, &org, "Idempotent Del Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    // First delete succeeds
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Second delete returns 404 (project no longer visible)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp2.status(), StatusCode::NOT_FOUND);
}

// Admin can hard-delete an already soft-deleted project
#[tokio::test]
async fn projects_hard_delete_soft_deleted_project() {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin User", "projhardsoftdel@example.com")
        .await;
    let org = server.create_org(&admin, "Hard Soft Del Org").await;
    let project = server
        .create_project(&admin, &org, "Hard Soft Del Project")
        .await;

    // Soft-delete first
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Hard-delete by UUID (slug is mangled)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{}?hard=true", project.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    // Hard delete can find soft-deleted entities
    assert_eq!(resp2.status(), StatusCode::NO_CONTENT);
}

// PATCH by UUID returns 404 after soft-delete
#[tokio::test]
async fn projects_patch_after_soft_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projpatchsd@example.com").await;
    let org = server.create_org(&user, "Patch After SD Org").await;
    let project = server
        .create_project(&user, &org, "Patch After SD Project")
        .await;

    // Soft-delete
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // PATCH by UUID should return 404
    let body = serde_json::json!({ "name": "Should Fail" });
    let patch_resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", project.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(patch_resp.status(), StatusCode::NOT_FOUND);
}

// Admin hard-delete of nonexistent UUID returns 404
#[tokio::test]
async fn projects_hard_delete_nonexistent() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin User", "projhardne@example.com").await;

    let fake_uuid = ProjectUuid::new();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{fake_uuid}?hard=true")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Soft-delete project, verify child resource endpoints return 404
#[tokio::test]
async fn projects_soft_delete_endpoints_inaccessible() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projsdchild@example.com").await;
    let org = server.create_org(&user, "Child Res Org").await;
    let project = server
        .create_project(&user, &org, "Child Res Project")
        .await;

    let project_slug: &str = project.slug.as_ref();

    // Verify project is accessible before delete
    let pre_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(pre_resp.status(), StatusCode::OK);

    // Soft-delete the project
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let child_endpoints = [
        "branches",
        "testbeds",
        "measures",
        "benchmarks",
        "reports",
        "thresholds",
        "alerts",
        "plots",
    ];
    for endpoint in &child_endpoints {
        let child_resp = server
            .client
            .get(server.api_url(&format!("/v0/projects/{project_slug}/{endpoint}")))
            .header("Authorization", server.bearer(&user.token))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(
            child_resp.status(),
            StatusCode::NOT_FOUND,
            "{endpoint} should return 404 after project soft-delete"
        );
    }
}
