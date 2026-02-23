#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for organization CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonNewOrganization, JsonOrganization, JsonOrganizations};
use http::StatusCode;

// GET /v0/organizations - requires auth
#[tokio::test]
async fn organizations_list_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// GET /v0/organizations - authenticated user sees their orgs
#[tokio::test]
async fn organizations_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orglist@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let orgs: JsonOrganizations = resp.json().await.expect("Failed to parse response");
    // User should have their personal org created during signup
    assert!(!orgs.0.is_empty());
}

// POST /v0/organizations - create new org
#[tokio::test]
async fn organizations_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgcreate@example.com").await;

    let body = JsonNewOrganization {
        name: "Test Organization".parse().unwrap(),
        slug: Some("test-organization".parse().unwrap()),
    };

    let resp = server
        .client
        .post(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let org: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(org.name.as_ref(), "Test Organization");
}

// POST /v0/organizations - create with auto-generated slug
#[tokio::test]
async fn organizations_create_auto_slug() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgautoslug@example.com").await;

    let body = JsonNewOrganization {
        name: "Auto Slug Org".parse().unwrap(),
        slug: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let org: JsonOrganization = resp.json().await.expect("Failed to parse response");
    // Slug should be auto-generated
    let slug: &str = org.slug.as_ref();
    assert!(!slug.is_empty());
}

// GET /v0/organizations/{organization} - view org
#[tokio::test]
async fn organizations_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgget@example.com").await;
    let org = server.create_org(&user, "View Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, org.uuid);
}

// GET /v0/organizations/{organization} - not found
#[tokio::test]
async fn organizations_get_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgnotfound@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations/nonexistent-org"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// PATCH /v0/organizations/{organization} - update org
#[tokio::test]
async fn organizations_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgupdate@example.com").await;
    let org = server.create_org(&user, "Update Org").await;

    let body = serde_json::json!({
        "name": "Updated Org Name"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Org Name");
}

// DELETE /v0/organizations/{organization} - delete org
#[tokio::test]
async fn organizations_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgdelete@example.com").await;
    let org = server.create_org(&user, "Delete Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify org is deleted
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// Soft-delete removes org from list
#[tokio::test]
async fn organizations_soft_delete_not_in_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgsoftdel@example.com").await;
    let org = server.create_org(&user, "Soft Delete Org").await;

    // Soft-delete (default)
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify absent from list
    let list_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let orgs: JsonOrganizations = list_resp.json().await.expect("Failed to parse response");
    assert!(
        !orgs.0.iter().any(|o| o.uuid == org.uuid),
        "Soft-deleted org should not appear in list"
    );
}

// Soft-delete org cascades to child projects
#[tokio::test]
async fn organizations_soft_delete_cascades_to_projects() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgcascade@example.com").await;
    let org = server.create_org(&user, "Cascade Org").await;
    let project = server.create_project(&user, &org, "Cascade Project").await;

    // Soft-delete the org
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify child project returns 404
    let project_slug: &str = project.slug.as_ref();
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// Soft-delete frees slug for reuse
#[tokio::test]
async fn organizations_soft_delete_slug_reuse() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgslugresue@example.com").await;
    let org = server.create_org(&user, "Slug Reuse Org").await;

    // Soft-delete
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Create new org with the same slug
    let body = JsonNewOrganization {
        name: "Slug Reuse Org".parse().unwrap(),
        slug: Some(org.slug.clone()),
    };
    let create_resp = server
        .client
        .post(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
}

// Hard delete requires server admin
#[tokio::test]
async fn organizations_hard_delete_requires_admin() {
    let server = TestServer::new().await;
    // First signup is admin
    let _admin = server.signup("Admin", "orghardadm@example.com").await;
    // Second signup is NOT admin
    let user = server
        .signup("Regular User", "orgharduser@example.com")
        .await;
    let org = server.create_org(&user, "Hard Delete Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}?hard=true")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// Admin can hard-delete
#[tokio::test]
async fn organizations_hard_delete_as_admin() {
    let server = TestServer::new().await;
    // First signup is admin
    let admin = server.signup("Admin User", "orghardok@example.com").await;
    let org = server.create_org(&admin, "Admin Hard Del Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}?hard=true")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify truly gone
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// GET by UUID returns 404 after soft-delete
#[tokio::test]
async fn organizations_soft_delete_get_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgsduuid@example.com").await;
    let org = server.create_org(&user, "UUID Delete Org").await;

    // Soft-delete
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET by UUID should return 404
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}", org.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
}

// Second soft-delete returns 404 (idempotent)
#[tokio::test]
async fn organizations_soft_delete_idempotent() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "orgsdidempotent@example.com")
        .await;
    let org = server.create_org(&user, "Idempotent Del Org").await;

    let org_slug: &str = org.slug.as_ref();
    // First delete succeeds
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Second delete returns 404 (org no longer visible)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp2.status(), StatusCode::NOT_FOUND);
}

// Admin can hard-delete an already soft-deleted org
#[tokio::test]
async fn organizations_hard_delete_soft_deleted_org() {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin User", "orghardsoftdel@example.com")
        .await;
    let org = server.create_org(&admin, "Hard Soft Del Org").await;

    // Soft-delete first
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Hard-delete by UUID (slug is mangled)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}?hard=true", org.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    // Org is soft-deleted so it's not found via normal lookup
    assert_eq!(resp2.status(), StatusCode::NOT_FOUND);
}

// Soft-delete org, verify child resource list endpoints return 404
#[tokio::test]
async fn organizations_soft_delete_cascades_child_resources() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgsdchild@example.com").await;
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

    // Soft-delete the org
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{org_slug}")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // All child resource endpoints under the project should return 404
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
            "{endpoint} should return 404 after org soft-delete"
        );
    }
}
