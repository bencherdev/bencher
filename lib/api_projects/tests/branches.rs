#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::similar_names,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project branch endpoints.

use bencher_api_tests::{TestServer, TestUser};
use bencher_json::{HeadUuid, JsonBranch, JsonBranches, VersionUuid};
use bencher_schema::{context::DbConnection, schema};
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

async fn send_branch_json(
    server: &TestServer,
    user: &TestUser,
    method: http::Method,
    path: &str,
    body: &serde_json::Value,
) -> StatusCode {
    server
        .client
        .request(method, server.api_url(path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed")
        .status()
}

fn branch_head_id(conn: &mut DbConnection, slug: &str) -> Option<i32> {
    schema::branch::table
        .filter(schema::branch::slug.eq(slug))
        .select(schema::branch::head_id)
        .first(conn)
        .expect("Failed to get branch head")
}

fn head_version_ids(conn: &mut DbConnection, head_id: i32) -> Vec<i32> {
    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .order(schema::head_version::version_id)
        .select(schema::head_version::version_id)
        .load(conn)
        .expect("Failed to get head versions")
}

fn seed_versions(conn: &mut DbConnection, slug: &str, count: i32) -> Vec<i32> {
    let (project_id, head_id): (i32, Option<i32>) = schema::branch::table
        .filter(schema::branch::slug.eq(slug))
        .select((schema::branch::project_id, schema::branch::head_id))
        .first(conn)
        .expect("Failed to get branch");
    let head_id = head_id.expect("Branch has no head");
    for number in 1..=count {
        let version_uuid = VersionUuid::new();
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(&version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(number),
            ))
            .execute(conn)
            .expect("Failed to insert version");
        let version_id: i32 = schema::version::table
            .filter(schema::version::uuid.eq(&version_uuid))
            .select(schema::version::id)
            .first(conn)
            .expect("Failed to get version ID");
        diesel::insert_into(schema::head_version::table)
            .values((
                schema::head_version::head_id.eq(head_id),
                schema::head_version::version_id.eq(version_id),
            ))
            .execute(conn)
            .expect("Failed to insert head version");
    }
    head_version_ids(conn, head_id)
}

fn fail_head_version_inserts(conn: &mut DbConnection, fail: bool) {
    let sql = if fail {
        "CREATE TRIGGER fail_head_version BEFORE INSERT ON head_version BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
    } else {
        "DROP TRIGGER fail_head_version"
    };
    diesel::sql_query(sql)
        .execute(conn)
        .expect("Failed to toggle trigger");
}

// POST /v0/projects/{project}/branches - a start point clone failure leaves no branch behind
#[tokio::test]
async fn branches_create_with_start_point_is_atomic() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchstart@example.com").await;
    let org = server.create_org(&user, "Branch Start Org").await;
    let project = server
        .create_project(&user, &org, "Branch Start Project")
        .await;
    let path = format!("/v0/projects/{}/branches", project.slug);

    let source = serde_json::json!({ "name": "source", "slug": "source" });
    let status = send_branch_json(&server, &user, http::Method::POST, &path, &source).await;
    assert_eq!(status, StatusCode::CREATED);
    let mut conn = server.db_conn();
    let source_versions = seed_versions(&mut conn, "source", 3);

    let feature = serde_json::json!({
        "name": "feature",
        "slug": "feature",
        "start_point": { "branch": "source" }
    });
    fail_head_version_inserts(&mut conn, true);
    let status = send_branch_json(&server, &user, http::Method::POST, &path, &feature).await;
    assert!(!status.is_success(), "{status}");
    let feature_count: i64 = schema::branch::table
        .filter(schema::branch::slug.eq("feature"))
        .count()
        .get_result(&mut conn)
        .expect("Failed to count branches");
    assert_eq!(feature_count, 0);

    fail_head_version_inserts(&mut conn, false);
    let status = send_branch_json(&server, &user, http::Method::POST, &path, &feature).await;
    assert_eq!(status, StatusCode::CREATED);
    let head_id = branch_head_id(&mut conn, "feature").expect("Branch has no head");
    assert_eq!(head_version_ids(&mut conn, head_id), source_versions);
}

// PATCH /v0/projects/{project}/branches/{branch} - a start point clone failure keeps the old head
#[tokio::test]
async fn branches_reset_start_point_is_atomic() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchreset@example.com").await;
    let org = server.create_org(&user, "Branch Reset Org").await;
    let project = server
        .create_project(&user, &org, "Branch Reset Project")
        .await;
    let path = format!("/v0/projects/{}/branches", project.slug);

    for slug in ["source", "feature"] {
        let body = serde_json::json!({ "name": slug, "slug": slug });
        let status = send_branch_json(&server, &user, http::Method::POST, &path, &body).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let mut conn = server.db_conn();
    let source_versions = seed_versions(&mut conn, "source", 3);
    let old_head_id = branch_head_id(&mut conn, "feature");

    let path = format!("{path}/feature");
    let reset = serde_json::json!({ "start_point": { "branch": "source", "reset": true } });
    fail_head_version_inserts(&mut conn, true);
    let status = send_branch_json(&server, &user, http::Method::PATCH, &path, &reset).await;
    assert!(!status.is_success(), "{status}");
    assert_eq!(branch_head_id(&mut conn, "feature"), old_head_id);

    fail_head_version_inserts(&mut conn, false);
    let status = send_branch_json(&server, &user, http::Method::PATCH, &path, &reset).await;
    assert_eq!(status, StatusCode::OK);
    let head_id = branch_head_id(&mut conn, "feature").expect("Branch has no head");
    assert_ne!(Some(head_id), old_head_id);
    assert_eq!(head_version_ids(&mut conn, head_id), source_versions);
}
