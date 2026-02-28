#![expect(
    unused_crate_dependencies,
    clippy::similar_names,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project branch endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{HeadUuid, JsonBranch, JsonBranches};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// GET /v0/projects/{project}/branches/{branch}?head= - view branch with specific head
#[tokio::test]
async fn branches_get_with_head_query() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchhead@example.com").await;
    let org = server.create_org(&user, "Branch Head Org").await;
    let project = server
        .create_project(&user, &org, "Branch Head Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create a branch — this auto-creates a head (head A)
    let body = serde_json::json!({
        "name": "head-branch",
        "slug": "head-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");
    let head_a_uuid = branch.head.uuid;

    // Insert a second head (head B) for the same branch directly in the DB
    let head_b_uuid = HeadUuid::new();
    let mut conn = server.db_conn();
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(branch.uuid.to_string()))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to get branch ID");
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(head_b_uuid.to_string()),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(0i64),
        ))
        .execute(&mut conn)
        .expect("Failed to insert second head");
    drop(conn);

    // GET without ?head= returns current head (head A)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/head-branch",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");
    assert_eq!(branch.head.uuid, head_a_uuid);

    // GET with ?head=<head_a_uuid> returns head A explicitly
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/head-branch?head={}",
            project_slug, head_a_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");
    assert_eq!(branch.head.uuid, head_a_uuid);

    // GET with ?head=<head_b_uuid> returns head B (historical)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/head-branch?head={}",
            project_slug, head_b_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");
    assert_eq!(branch.head.uuid, head_b_uuid);
}

// GET /v0/projects/{project}/branches/{branch}?head=<nonexistent> - returns 404
#[tokio::test]
async fn branches_get_with_nonexistent_head() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchheadnf@example.com").await;
    let org = server.create_org(&user, "Branch HeadNF Org").await;
    let project = server
        .create_project(&user, &org, "Branch HeadNF Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create a branch
    let body = serde_json::json!({
        "name": "headnf-branch",
        "slug": "headnf-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // GET with a nonexistent head UUID should return 404
    let bogus_uuid = HeadUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/headnf-branch?head={}",
            project_slug, bogus_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/branches/{branch}?head=<other_branch_head> - returns 404
#[tokio::test]
async fn branches_get_with_wrong_branch_head() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "branchheadwrong@example.com")
        .await;
    let org = server.create_org(&user, "Branch HeadWrong Org").await;
    let project = server
        .create_project(&user, &org, "Branch HeadWrong Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create branch A
    let body = serde_json::json!({
        "name": "branch-a",
        "slug": "branch-a"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create branch B
    let body = serde_json::json!({
        "name": "branch-b",
        "slug": "branch-b"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch_b: JsonBranch = resp.json().await.expect("Failed to parse branch B");
    let branch_b_head_uuid = branch_b.head.uuid;

    // GET branch A with branch B's head UUID should return 404
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/branch-a?head={}",
            project_slug, branch_b_head_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
