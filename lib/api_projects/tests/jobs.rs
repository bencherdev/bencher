#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix
)]
//! Integration tests for project job endpoints.
//!
//! Note: Jobs are created when reports are submitted, so most tests
//! verify correct behavior with empty job lists and proper error handling.

use bencher_api_tests::TestServer;
use bencher_json::runner::JsonJobs;
use http::StatusCode;

// GET /v0/projects/{project}/jobs - list jobs (empty)
#[tokio::test]
async fn test_jobs_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "joblist@example.com").await;
    let org = server.create_org(&user, "Job Org").await;
    let project = server.create_project(&user, &org, "Job Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    // New project should have no jobs
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with pagination
#[tokio::test]
async fn test_jobs_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpage@example.com").await;
    let org = server.create_org(&user, "Job Page Org").await;
    let project = server.create_project(&user, &org, "Job Page Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs?per_page=10&page=1"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with status filter
#[tokio::test]
async fn test_jobs_list_with_status_filter() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobstatus@example.com").await;
    let org = server.create_org(&user, "Job Status Org").await;
    let project = server
        .create_project(&user, &org, "Job Status Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?status=pending")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with ascending order
#[tokio::test]
async fn test_jobs_list_ascending_order() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobasc@example.com").await;
    let org = server.create_org(&user, "Job Asc Org").await;
    let project = server.create_project(&user, &org, "Job Asc Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?direction=asc")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs/{job} - not found
#[tokio::test]
async fn test_jobs_get_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobnotfound@example.com").await;
    let org = server.create_org(&user, "Job NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Job NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs/00000000-0000-0000-0000-000000000000"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - public project, no auth
#[tokio::test]
async fn test_jobs_list_public_no_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpublic@example.com").await;
    let org = server.create_org(&user, "Job Public Org").await;
    let project = server
        .create_project(&user, &org, "Job Public Project")
        .await;

    // Projects are public by default
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs/{job} - public project, no auth, not found
#[tokio::test]
async fn test_jobs_get_public_no_auth_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpub2@example.com").await;
    let org = server.create_org(&user, "Job Public2 Org").await;
    let project = server
        .create_project(&user, &org, "Job Public2 Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs/00000000-0000-0000-0000-000000000000"
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - nonexistent project
#[tokio::test]
async fn test_jobs_list_project_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobnoproj@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects/nonexistent-project/jobs"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - X-Total-Count header present
#[tokio::test]
async fn test_jobs_list_total_count_header() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobtotal@example.com").await;
    let org = server.create_org(&user, "Job Total Org").await;
    let project = server
        .create_project(&user, &org, "Job Total Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    // Check that X-Total-Count header is present
    let total_count = resp.headers().get("X-Total-Count");
    assert!(total_count.is_some());
    assert_eq!(total_count.unwrap().to_str().unwrap(), "0");
}

// GET /v0/projects/{project}/jobs - using project UUID instead of slug
#[tokio::test]
async fn test_jobs_list_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobuuid@example.com").await;
    let org = server.create_org(&user, "Job UUID Org").await;
    let project = server.create_project(&user, &org, "Job UUID Project").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/jobs", project.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs - private project, no auth, should be denied
#[tokio::test]
async fn test_private_project_jobs_denied_unauthenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobprivate@example.com").await;
    let org = server.create_org(&user, "Job Private Org").await;
    let project = server
        .create_project(&user, &org, "Job Private Project")
        .await;

    // Make the project private by updating visibility directly in the database
    {
        use bencher_json::project::Visibility;
        use bencher_schema::schema;
        use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    // Request jobs without auth - should be denied
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .send()
        .await
        .expect("Request failed");

    // Private project should deny unauthenticated access
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN for private project, got {}",
        resp.status()
    );
}

// =============================================================================
// Tests with actual data
// =============================================================================

/// Helper: get project_id from project slug.
#[expect(clippy::expect_used)]
fn get_project_id(server: &TestServer, project_slug: &str) -> i32 {
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::slug.eq(project_slug))
        .select(schema::project::id)
        .first(&mut conn)
        .expect("Failed to get project ID")
}

/// Helper: create minimal test infrastructure (testbed, version, branch, head, report).
/// Returns the report ID.
///
/// Note: A similar helper exists in `plus/api_runners/tests/common/mod.rs`.
/// They are kept separate due to crate boundaries and slightly different signatures.
#[expect(clippy::expect_used)]
fn create_test_report(server: &TestServer, project_id: i32) -> i32 {
    use bencher_json::{BranchUuid, DateTime, HeadUuid, ReportUuid, TestbedUuid, VersionUuid};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    let now = DateTime::now();

    let testbed_uuid = TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq("test-testbed"),
            schema::testbed::slug.eq(&format!("test-testbed-{testbed_uuid}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert testbed");
    let testbed_id: i32 = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&testbed_uuid))
        .select(schema::testbed::id)
        .first(&mut conn)
        .expect("Failed to get testbed ID");

    let version_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut conn)
        .expect("Failed to insert version");
    let version_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("Failed to get version ID");

    let branch_uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(&branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq("main"),
            schema::branch::slug.eq(&format!("main-{branch_uuid}")),
            schema::branch::created.eq(&now),
            schema::branch::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert branch");
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(&branch_uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to get branch ID");

    let head_uuid = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(&head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert head");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(&head_uuid))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("Failed to get head ID");

    let report_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&report_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(head_id),
            schema::report::version_id.eq(version_id),
            schema::report::testbed_id.eq(testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&now),
            schema::report::end_time.eq(&now),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert report");

    schema::report::table
        .filter(schema::report::uuid.eq(&report_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("Failed to get report ID")
}

/// Helper: insert a test job into the database with a specific created timestamp.
/// Returns the job UUID.
#[expect(clippy::expect_used)]
fn insert_test_job(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    created: bencher_json::DateTime,
) -> bencher_json::JobUuid {
    use bencher_json::{JobPriority, JobStatus, JobUuid, SpecUuid};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    let job_uuid = JobUuid::new();

    // Create a spec row for hardware requirements
    let spec_uuid = SpecUuid::new();
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::cpu.eq(2),
            schema::spec::memory.eq(4_294_967_296_i64),
            schema::spec::disk.eq(10_737_418_240_i64),
            schema::spec::network.eq(false),
            schema::spec::created.eq(&created),
            schema::spec::modified.eq(&created),
        ))
        .execute(&mut conn)
        .expect("Failed to insert spec");
    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec_uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to get spec ID");

    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": 3600
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(1),
            schema::job::source_ip.eq("127.0.0.1"),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(JobPriority::default()),
            schema::job::created.eq(&created),
            schema::job::modified.eq(&created),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    job_uuid
}

/// Helper: set job status directly in the database.
#[expect(clippy::expect_used)]
fn set_job_status(
    server: &TestServer,
    job_uuid: bencher_json::JobUuid,
    status: bencher_json::JobStatus,
) {
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::status.eq(status))
        .execute(&mut conn)
        .expect("Failed to set job status");
}

// GET /v0/projects/{project}/jobs - list returns inserted jobs
#[tokio::test]
async fn test_jobs_list_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobdata@example.com").await;
    let org = server.create_org(&user, "Job Data Org").await;
    let project = server.create_project(&user, &org, "Job Data Project").await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = bencher_json::DateTime::now();

    let _job1 = insert_test_job(&server, report_id, project.uuid, now);
    let _job2 = insert_test_job(&server, report_id, project.uuid, now);
    let _job3 = insert_test_job(&server, report_id, project.uuid, now);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 3);
}

// GET /v0/projects/{project}/jobs - pagination with data
#[tokio::test]
async fn test_jobs_list_pagination_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpagedata@example.com").await;
    let org = server.create_org(&user, "Job PageData Org").await;
    let project = server
        .create_project(&user, &org, "Job PageData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = bencher_json::DateTime::now();

    let _job1 = insert_test_job(&server, report_id, project.uuid, now);
    let _job2 = insert_test_job(&server, report_id, project.uuid, now);
    let _job3 = insert_test_job(&server, report_id, project.uuid, now);

    let project_slug: &str = project.slug.as_ref();

    // Request first page with per_page=2
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs?per_page=2&page=1"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 2);

    // Request second page
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs?per_page=2&page=2"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 1);
}

// GET /v0/projects/{project}/jobs - status filter with data
#[tokio::test]
async fn test_jobs_list_status_filter_with_data() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "jobfilterdata@example.com")
        .await;
    let org = server.create_org(&user, "Job FilterData Org").await;
    let project = server
        .create_project(&user, &org, "Job FilterData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = bencher_json::DateTime::now();

    let job1 = insert_test_job(&server, report_id, project.uuid, now);
    let _job2 = insert_test_job(&server, report_id, project.uuid, now);
    let job3 = insert_test_job(&server, report_id, project.uuid, now);

    // Set job1 to Running and job3 to Completed
    set_job_status(&server, job1, bencher_json::JobStatus::Running);
    set_job_status(&server, job3, bencher_json::JobStatus::Completed);

    let project_slug: &str = project.slug.as_ref();

    // Filter by pending - should only return job2
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?status=pending")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 1);
    assert_eq!(jobs.0[0].status, bencher_json::JobStatus::Pending);

    // Filter by running - should only return job1
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?status=running")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 1);
    assert_eq!(jobs.0[0].status, bencher_json::JobStatus::Running);
}

// GET /v0/projects/{project}/jobs - ordering with data
#[tokio::test]
async fn test_jobs_list_ordering_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "joborderdata@example.com").await;
    let org = server.create_org(&user, "Job OrderData Org").await;
    let project = server
        .create_project(&user, &org, "Job OrderData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let base_ts = bencher_json::DateTime::now();
    let ts1 = base_ts;
    let ts2 = bencher_json::DateTime::try_from(base_ts.timestamp() + 1).unwrap();
    let ts3 = bencher_json::DateTime::try_from(base_ts.timestamp() + 2).unwrap();

    let job1 = insert_test_job(&server, report_id, project.uuid, ts1);
    let _job2 = insert_test_job(&server, report_id, project.uuid, ts2);
    let job3 = insert_test_job(&server, report_id, project.uuid, ts3);

    let project_slug: &str = project.slug.as_ref();

    // Default order is created desc
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 3);
    assert_eq!(
        jobs.0[0].uuid, job3,
        "Most recent job should be first (desc)"
    );
    assert_eq!(jobs.0[2].uuid, job1, "Oldest job should be last (desc)");

    // Ascending order
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?direction=asc")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 3);
    assert_eq!(jobs.0[0].uuid, job1, "Oldest job should be first (asc)");
    assert_eq!(jobs.0[2].uuid, job3, "Most recent job should be last (asc)");
}

// GET /v0/projects/{project}/jobs - X-Total-Count with data
#[tokio::test]
async fn test_jobs_total_count_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobtotaldata@example.com").await;
    let org = server.create_org(&user, "Job TotalData Org").await;
    let project = server
        .create_project(&user, &org, "Job TotalData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = bencher_json::DateTime::now();

    let _job1 = insert_test_job(&server, report_id, project.uuid, now);
    let _job2 = insert_test_job(&server, report_id, project.uuid, now);
    let _job3 = insert_test_job(&server, report_id, project.uuid, now);

    let project_slug: &str = project.slug.as_ref();

    // Request with per_page=2 but total count should still be 3
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs?per_page=2&page=1"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let total_count = resp
        .headers()
        .get("X-Total-Count")
        .expect("X-Total-Count header missing");
    assert_eq!(total_count.to_str().unwrap(), "3");

    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 2);
}

// GET /v0/projects/{project}/jobs - non-member cannot access private project's jobs
#[tokio::test]
async fn test_non_member_private_project_jobs() {
    let server = TestServer::new().await;
    let owner = server.signup("Owner", "jobprivowner@example.com").await;
    let non_member = server
        .signup("NonMember", "jobprivnonmem@example.com")
        .await;

    let org = server.create_org(&owner, "Job Priv Owner Org").await;
    let project = server
        .create_project(&owner, &org, "Job Priv Owner Project")
        .await;

    // Make the project private
    {
        use bencher_json::project::Visibility;
        use bencher_schema::schema;
        use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    // Insert some jobs so there's data to potentially leak
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = bencher_json::DateTime::now();
    let _job = insert_test_job(&server, report_id, project.uuid, now);

    // Non-member tries to access the private project's jobs
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&non_member.token))
        .send()
        .await
        .expect("Request failed");

    // Non-member should be denied access to a private project
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN for non-member, got {}",
        resp.status()
    );
}
