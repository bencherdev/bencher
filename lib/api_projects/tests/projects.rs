#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify project is deleted
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify absent from list
    let list_resp = server
        .client
        .get(server.api_url("/v0/projects"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .signup("Test User", "projslugreuse@example.com")
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify truly gone
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET by UUID should return 404
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Second delete returns 404 (project no longer visible)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Hard-delete by UUID (slug is mangled)
    let resp2 = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{}?hard=true", project.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // PATCH by UUID should return 404
    let body = serde_json::json!({ "name": "Should Fail" });
    let patch_resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", project.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn non_member_patch_public_project_returns_403() {
    let server = TestServer::new().await;
    let owner = server
        .signup("Owner", "projpatchpubowner@example.com")
        .await;
    let outsider = server
        .signup("Outsider", "projpatchpubother@example.com")
        .await;
    let org = server.create_org(&owner, "Patch Pub Org").await;
    let project = server
        .create_project(&owner, &org, "Patch Pub Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({ "name": "Hijacked" });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&outsider.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp_body = resp.text().await.expect("Failed to read response body");
    assert!(
        resp_body.contains("access denied"),
        "Expected 'access denied' in body, got: {}",
        resp_body
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn non_member_patch_private_project_returns_404() {
    use bencher_json::project::Visibility;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let owner = server
        .signup("Owner", "projpatchprivowner@example.com")
        .await;
    let outsider = server
        .signup("Outsider", "projpatchprivother@example.com")
        .await;
    let org = server.create_org(&owner, "Patch Priv Org").await;
    let project = server
        .create_project(&owner, &org, "Patch Priv Project")
        .await;

    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({ "name": "Hijacked" });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&outsider.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp_body = resp.text().await.expect("Failed to read response body");
    assert!(
        resp_body.contains("may be private"),
        "Expected info-hiding wording in body, got: {}",
        resp_body
    );
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(pre_resp.status(), StatusCode::OK);

    // Soft-delete the project
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
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

// Plot dimensions seeded directly in the DB for the plot PATCH test.
struct PlotDimensions {
    branch1: bencher_json::BranchUuid,
    branch2: bencher_json::BranchUuid,
    testbed: bencher_json::TestbedUuid,
    benchmark: bencher_json::BenchmarkUuid,
    measure: bencher_json::MeasureUuid,
}

#[expect(clippy::expect_used, reason = "test helper seeding plot dimensions")]
fn seed_plot_dimensions(server: &TestServer, project_id: i32) -> PlotDimensions {
    use bencher_api_tests::helpers::base_timestamp;
    use bencher_json::{BenchmarkUuid, BranchUuid, MeasureUuid, TestbedUuid};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, RunQueryDsl as _};

    let now = base_timestamp();
    let branch1 = BranchUuid::new();
    let branch2 = BranchUuid::new();
    let testbed = TestbedUuid::new();
    let benchmark = BenchmarkUuid::new();
    let measure = MeasureUuid::new();

    let mut conn = server.db_conn();
    for (uuid, name) in [(&branch1, "branch-one"), (&branch2, "branch-two")] {
        diesel::insert_into(schema::branch::table)
            .values((
                schema::branch::uuid.eq(uuid),
                schema::branch::project_id.eq(project_id),
                schema::branch::name.eq(name),
                schema::branch::slug.eq(&format!("{name}-{uuid}")),
                schema::branch::created.eq(&now),
                schema::branch::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert branch");
    }
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq("testbed-one"),
            schema::testbed::slug.eq(&format!("testbed-one-{testbed}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert testbed");
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("benchmark-one"),
            schema::benchmark::slug.eq(&format!("benchmark-one-{benchmark}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert benchmark");
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq("latency"),
            schema::measure::slug.eq(&format!("latency-{measure}")),
            schema::measure::units.eq("nanoseconds"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert measure");

    PlotDimensions {
        branch1,
        branch2,
        testbed,
        benchmark,
        measure,
    }
}

// PATCH /v0/projects/{project}/plots/{plot} - full update (flags, x-axis, components)
#[tokio::test]
async fn plot_patch_updates_full_config() {
    use bencher_api_tests::helpers::get_project_id;
    use bencher_json::{JsonPlot, project::plot::XAxis};

    let server = TestServer::new().await;
    let user = server.signup("Plot User", "plotpatch@example.com").await;
    let org = server.create_org(&user, "Plot Org").await;
    let project = server.create_project(&user, &org, "Plot Project").await;
    let project_slug: &str = project.slug.as_ref();
    let project_id = get_project_id(&server, project_slug);

    let PlotDimensions {
        branch1,
        branch2,
        testbed,
        benchmark,
        measure,
    } = seed_plot_dimensions(&server, project_id);

    // Create the plot via the API.
    let new_plot = serde_json::json!({
        "lower_value": true,
        "upper_value": true,
        "lower_boundary": false,
        "upper_boundary": false,
        "x_axis": "date_time",
        "window": 2_592_000,
        "branches": [branch1.to_string()],
        "testbeds": [testbed.to_string()],
        "benchmarks": [benchmark.to_string()],
        "measures": [measure.to_string()],
    });
    let created: JsonPlot = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/plots")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&new_plot)
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse created plot");
    assert_eq!(created.branches, vec![branch1]);
    assert!(created.lower_value);
    assert!(matches!(created.x_axis, XAxis::DateTime));

    // PATCH: flip a flag, switch the x-axis, and replace the branches.
    let patch = serde_json::json!({
        "lower_value": false,
        "x_axis": "version",
        "branches": [branch1.to_string(), branch2.to_string()],
    });
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{project_slug}/plots/{}",
            created.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&patch)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonPlot = resp.json().await.expect("Failed to parse updated plot");

    // Provided fields changed.
    assert!(!updated.lower_value);
    assert!(matches!(updated.x_axis, XAxis::Version));
    assert_eq!(updated.branches, vec![branch1, branch2]);
    // Omitted fields left unchanged.
    assert!(updated.upper_value);
    assert_eq!(updated.testbeds, vec![testbed]);
    assert_eq!(updated.benchmarks, vec![benchmark]);
    assert_eq!(updated.measures, vec![measure]);
}

// PATCH referencing a component that does not belong to the project is rejected.
#[tokio::test]
async fn plot_patch_rejects_unknown_component() {
    use bencher_api_tests::helpers::get_project_id;
    use bencher_json::{BranchUuid, JsonPlot};

    let server = TestServer::new().await;
    let user = server
        .signup("Plot User 404", "plotpatch404@example.com")
        .await;
    let org = server.create_org(&user, "Plot Org 404").await;
    let project = server.create_project(&user, &org, "Plot Project 404").await;
    let project_slug: &str = project.slug.as_ref();
    let project_id = get_project_id(&server, project_slug);

    let dims = seed_plot_dimensions(&server, project_id);
    let new_plot = serde_json::json!({
        "lower_value": true,
        "upper_value": true,
        "lower_boundary": false,
        "upper_boundary": false,
        "x_axis": "date_time",
        "window": 2_592_000,
        "branches": [dims.branch1.to_string()],
        "testbeds": [dims.testbed.to_string()],
        "benchmarks": [dims.benchmark.to_string()],
        "measures": [dims.measure.to_string()],
    });
    let created: JsonPlot = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/plots")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&new_plot)
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse created plot");

    // A branch UUID that was never created in this project must be rejected
    // rather than silently referenced or returning a 500.
    let unknown = BranchUuid::new();
    let patch = serde_json::json!({ "branches": [unknown.to_string()] });
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{project_slug}/plots/{}",
            created.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&patch)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
