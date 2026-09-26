#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::similar_names,
    reason = "integration test file"
)]
//! Integration tests for project job endpoints.
//!
//! Note: Jobs are created when reports are submitted, so most tests
//! verify correct behavior with empty job lists and proper error handling.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, create_test_report, get_project_id, set_job_status},
};
use bencher_json::runner::JsonJobs;
use bencher_schema::model::runner::{CallbackOutcome, JobId};
use http::StatusCode;

// GET /v0/projects/{project}/jobs - list jobs (empty)
#[tokio::test]
async fn jobs_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "joblist@example.com").await;
    let org = server.create_org(&user, "Job Org").await;
    let project = server.create_project(&user, &org, "Job Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
async fn jobs_list_with_pagination() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with status filter
#[tokio::test]
async fn jobs_list_with_status_filter() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with ascending order
#[tokio::test]
async fn jobs_list_ascending_order() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobasc@example.com").await;
    let org = server.create_org(&user, "Job Asc Org").await;
    let project = server.create_project(&user, &org, "Job Asc Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?direction=asc")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs/{job} - not found
#[tokio::test]
async fn jobs_get_not_found() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - public project, no auth
#[tokio::test]
async fn jobs_list_public_no_auth() {
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
async fn jobs_get_public_no_auth_not_found() {
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
async fn jobs_list_project_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobnoproj@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects/nonexistent-project/jobs"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - X-Total-Count header present
#[tokio::test]
async fn jobs_list_total_count_header() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
async fn jobs_list_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobuuid@example.com").await;
    let org = server.create_org(&user, "Job UUID Org").await;
    let project = server.create_project(&user, &org, "Job UUID Project").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/jobs", project.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs - private project, no auth, should be denied
#[tokio::test]
async fn private_project_jobs_denied_unauthenticated() {
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

/// Helper: insert a test job into the database with a specific created timestamp.
/// Returns the job UUID.
fn insert_test_job(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    created: bencher_json::DateTime,
) -> bencher_json::JobUuid {
    insert_test_job_with_timeout(
        server,
        report_id,
        project_uuid,
        created,
        bencher_json::Timeout::PLUS_DEFAULT,
    )
}

/// Helper: insert a test job with a specific timeout. Returns the job UUID.
#[expect(clippy::expect_used, reason = "test helper")]
fn insert_test_job_with_timeout(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    created: bencher_json::DateTime,
    timeout: bencher_json::Timeout,
) -> bencher_json::JobUuid {
    use bencher_json::{JobStatus, JobUuid, Priority, SpecUuid};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    let job_uuid = JobUuid::new();

    // Create a spec row for hardware requirements
    let spec_uuid = SpecUuid::new();
    let spec_name = format!("test-spec-{spec_uuid}");
    let spec_slug = format!("test-spec-{spec_uuid}");
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::name.eq(&spec_name),
            schema::spec::slug.eq(&spec_slug),
            schema::spec::os.eq("linux"),
            schema::spec::architecture.eq("x86_64"),
            schema::spec::cpu.eq(2),
            schema::spec::memory.eq(0x0001_0000_0000i64),
            schema::spec::disk.eq(0x0002_8000_0000i64),
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

    let project_id: i32 = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select(schema::report::project_id)
        .first(&mut conn)
        .expect("Failed to get project ID from report");
    let organization_id: i32 = schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(&mut conn)
        .expect("Failed to get organization ID");

    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": timeout
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq("127.0.0.1"),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(timeout),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&created),
            schema::job::modified.eq(&created),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Helper: insert another report on the same head, version, and testbed as `report_id`.
/// `create_test_report` cannot run twice in one project: its testbed and branch names are unique.
#[expect(clippy::expect_used, reason = "test helper")]
fn create_sibling_report(server: &TestServer, report_id: i32) -> i32 {
    use bencher_json::ReportUuid;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let mut conn = server.db_conn();
    let (project_id, head_id, version_id, testbed_id): (i32, i32, i32, i32) = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select((
            schema::report::project_id,
            schema::report::head_id,
            schema::report::version_id,
            schema::report::testbed_id,
        ))
        .first(&mut conn)
        .expect("Failed to get report");

    let now = base_timestamp();
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

#[expect(clippy::expect_used, reason = "test helper")]
fn get_report_uuid(server: &TestServer, report_id: i32) -> bencher_json::ReportUuid {
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select(schema::report::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to get report UUID")
}

// GET /v0/projects/{project}/jobs - list returns inserted jobs
#[tokio::test]
async fn jobs_list_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobdata@example.com").await;
    let org = server.create_org(&user, "Job Data Org").await;
    let project = server.create_project(&user, &org, "Job Data Project").await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();

    let _job1 = insert_test_job(&server, report_id, project.uuid, now);
    let _job2 = insert_test_job(&server, report_id, project.uuid, now);
    let _job3 = insert_test_job(&server, report_id, project.uuid, now);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 3);
}

// GET /v0/projects/{project}/jobs - each job carries its own report and timeout
#[tokio::test]
async fn jobs_list_report_and_timeout() {
    use bencher_json::Timeout;

    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "joblistreport@example.com")
        .await;
    let org = server.create_org(&user, "Job ListReport Org").await;
    let project = server
        .create_project(&user, &org, "Job ListReport Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_a = create_test_report(&server, project_id);
    let report_b = create_sibling_report(&server, report_a);
    let now = base_timestamp();

    let job_b =
        insert_test_job_with_timeout(&server, report_b, project.uuid, now, Timeout::FREE_MAX);
    let job_a =
        insert_test_job_with_timeout(&server, report_a, project.uuid, now, Timeout::UNCLAIMED_MAX);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 2);
    for (job_uuid, report_id, timeout) in [
        (job_a, report_a, Timeout::UNCLAIMED_MAX),
        (job_b, report_b, Timeout::FREE_MAX),
    ] {
        let job = jobs
            .0
            .iter()
            .find(|job| job.uuid == job_uuid)
            .expect("Job missing from list");
        assert_eq!(job.report, get_report_uuid(&server, report_id));
        assert_eq!(job.timeout, timeout);
    }
}

// GET /v0/projects/{project}/jobs - pagination with data
#[tokio::test]
async fn jobs_list_pagination_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpagedata@example.com").await;
    let org = server.create_org(&user, "Job PageData Org").await;
    let project = server
        .create_project(&user, &org, "Job PageData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert_eq!(jobs.0.len(), 1);
}

// GET /v0/projects/{project}/jobs - status filter with data
#[tokio::test]
async fn jobs_list_status_filter_with_data() {
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
    let now = base_timestamp();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
async fn jobs_list_ordering_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "joborderdata@example.com").await;
    let org = server.create_org(&user, "Job OrderData Org").await;
    let project = server
        .create_project(&user, &org, "Job OrderData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let base_ts = base_timestamp();
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
async fn jobs_total_count_with_data() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobtotaldata@example.com").await;
    let org = server.create_org(&user, "Job TotalData Org").await;
    let project = server
        .create_project(&user, &org, "Job TotalData Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
async fn non_member_private_project_jobs() {
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
    let now = base_timestamp();
    let _job = insert_test_job(&server, report_id, project.uuid, now);

    // Non-member tries to access the private project's jobs
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&non_member.token),
        )
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

// =============================================================================
// Single job GET tests (get_one_inner output-fetching path)
// =============================================================================

/// Helper: write job output JSON to the local OCI storage path.
///
/// The test server uses local filesystem OCI storage at `{db_parent}/oci/`.
/// Job output is stored at `{oci_dir}/{project_uuid}/output/v0/jobs/{job_uuid}`.
#[expect(clippy::expect_used, reason = "test helper")]
fn write_job_output(
    server: &TestServer,
    project_uuid: bencher_json::ProjectUuid,
    job_uuid: bencher_json::JobUuid,
    output: &serde_json::Value,
) {
    let oci_dir = server
        .db_path()
        .parent()
        .expect("db has parent")
        .join("registry");
    let output_dir = oci_dir
        .join(project_uuid.to_string())
        .join("output")
        .join("v0")
        .join("jobs");
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    let output_path = output_dir.join(job_uuid.to_string());
    std::fs::write(
        &output_path,
        serde_json::to_vec(output).expect("Failed to serialize"),
    )
    .expect("Failed to write job output");
}

// GET /v0/projects/{project}/jobs/{job} - pending job has no output
#[tokio::test]
async fn job_get_pending_no_output() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "jobgetpending@example.com")
        .await;
    let org = server.create_org(&user, "Job GetPending Org").await;
    let project = server
        .create_project(&user, &org, "Job GetPending Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let job_uuid = insert_test_job(&server, report_id, project.uuid, now);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.status, bencher_json::JobStatus::Pending);
    // Pending jobs should not have output fetched
    assert!(job.output.is_none());
}

// GET /v0/projects/{project}/jobs/{job} - the job's report and its own timeout
#[tokio::test]
async fn job_get_report_and_timeout() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobgetreport@example.com").await;
    let org = server.create_org(&user, "Job GetReport Org").await;
    let project = server
        .create_project(&user, &org, "Job GetReport Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_a = create_test_report(&server, project_id);
    let report_b = create_sibling_report(&server, report_a);
    let now = base_timestamp();
    let job_uuid = insert_test_job_with_timeout(
        &server,
        report_b,
        project.uuid,
        now,
        bencher_json::Timeout::FREE_MAX,
    );

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.report, get_report_uuid(&server, report_b));
    assert_eq!(job.timeout, bencher_json::Timeout::FREE_MAX);
}

// GET /v0/projects/{project}/jobs/{job} - completed job with stored output
#[tokio::test]
async fn job_get_completed_with_output() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "jobgetcompleted@example.com")
        .await;
    let org = server.create_org(&user, "Job GetCompleted Org").await;
    let project = server
        .create_project(&user, &org, "Job GetCompleted Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let job_uuid = insert_test_job(&server, report_id, project.uuid, now);

    // Set job to completed (terminal state)
    set_job_status(&server, job_uuid, bencher_json::JobStatus::Completed);

    // Write output to local OCI storage
    let output_json = serde_json::json!({
        "results": [{
            "exit_code": 0,
            "stdout": "benchmark results here",
            "stderr": "some warnings"
        }]
    });
    write_job_output(&server, project.uuid, job_uuid, &output_json);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.status, bencher_json::JobStatus::Completed);
    // Terminal job should have output fetched from blob storage
    let output = job.output.expect("Expected output for completed job");
    assert_eq!(output.results.len(), 1);
    assert_eq!(output.results[0].exit_code, 0);
    assert!(output.error.is_none());
    assert_eq!(
        output.results[0].stdout.as_deref(),
        Some("benchmark results here")
    );
    assert_eq!(output.results[0].stderr.as_deref(), Some("some warnings"));
}

// GET /v0/projects/{project}/jobs/{job} - completed job without stored output (graceful)
#[tokio::test]
async fn job_get_completed_no_stored_output() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "jobgetnooutput@example.com")
        .await;
    let org = server.create_org(&user, "Job GetNoOutput Org").await;
    let project = server
        .create_project(&user, &org, "Job GetNoOutput Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let job_uuid = insert_test_job(&server, report_id, project.uuid, now);

    // Set job to completed but do NOT write any output to storage
    set_job_status(&server, job_uuid, bencher_json::JobStatus::Completed);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.status, bencher_json::JobStatus::Completed);
    // Should gracefully return None when no output is stored
    assert!(job.output.is_none());
}

// GET /v0/projects/{project}/jobs/{job} - failed job with stored output
#[tokio::test]
async fn job_get_failed_with_output() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobgetfailed@example.com").await;
    let org = server.create_org(&user, "Job GetFailed Org").await;
    let project = server
        .create_project(&user, &org, "Job GetFailed Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let job_uuid = insert_test_job(&server, report_id, project.uuid, now);

    // Set job to failed (terminal state)
    set_job_status(&server, job_uuid, bencher_json::JobStatus::Failed);

    // Write error output to local OCI storage
    let output_json = serde_json::json!({
        "results": [{
            "exit_code": 1,
            "stdout": "partial output",
            "stderr": "fatal error: out of memory"
        }],
        "error": "container crashed"
    });
    write_job_output(&server, project.uuid, job_uuid, &output_json);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.status, bencher_json::JobStatus::Failed);
    // Failed jobs are terminal and should also have output fetched
    let output = job.output.expect("Expected output for failed job");
    assert_eq!(output.results.len(), 1);
    assert_eq!(output.results[0].exit_code, 1);
    assert_eq!(output.error.as_deref(), Some("container crashed"));
    assert_eq!(output.results[0].stdout.as_deref(), Some("partial output"));
    assert_eq!(
        output.results[0].stderr.as_deref(),
        Some("fatal error: out of memory")
    );
}

// GET /v0/projects/{project}/jobs/{job} - running job has no output
#[tokio::test]
async fn job_get_running_no_output() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "jobgetrunning@example.com")
        .await;
    let org = server.create_org(&user, "Job GetRunning Org").await;
    let project = server
        .create_project(&user, &org, "Job GetRunning Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let job_uuid = insert_test_job(&server, report_id, project.uuid, now);

    // Set job to running (non-terminal)
    set_job_status(&server, job_uuid, bencher_json::JobStatus::Running);

    // Even if output exists in storage, running jobs should not fetch it
    let output_json = serde_json::json!({
        "results": [{
            "exit_code": 0,
            "stdout": "should not be returned"
        }]
    });
    write_job_output(&server, project.uuid, job_uuid, &output_json);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let job: bencher_json::JsonJob = resp.json().await.expect("Failed to parse response");
    assert_eq!(job.uuid, job_uuid);
    assert_eq!(job.status, bencher_json::JobStatus::Running);
    // Running is non-terminal, so output should not be fetched
    assert!(job.output.is_none());
}

// =============================================================================
// Job callback tests
// =============================================================================

/// Sealed into every callback request below, so a response that leaks the request shows it.
const CALLBACK_MARKER: &str = "MARKER-3e8a61c4";

#[expect(clippy::expect_used, reason = "test helper")]
fn get_job_id(server: &TestServer, job_uuid: bencher_json::JobUuid) -> JobId {
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::id)
        .first(&mut server.db_conn())
        .expect("Failed to get job ID")
}

/// Helper: store a pending callback, sealed with the server's key, the way a submit does.
#[expect(clippy::expect_used, reason = "test helper")]
fn insert_pending_callback(server: &TestServer, job_uuid: bencher_json::JobUuid) -> JobId {
    use bencher_schema::model::runner::InsertJobCallback;

    let job_id = get_job_id(server, job_uuid);
    let request = serde_json::json!({
        "url": format!("https://example.com/{CALLBACK_MARKER}"),
        "headers": { "authorization": format!("Bearer {CALLBACK_MARKER}") },
        "body": format!("{{\"marker\": \"{CALLBACK_MARKER}\"}}"),
    });
    let sealed = server
        .context()
        .callback_key
        .seal(job_uuid, request.to_string().as_bytes())
        .expect("Failed to seal the callback request");
    InsertJobCallback::pending(job_id, sealed, base_timestamp())
        .insert(&mut server.db_conn())
        .expect("Failed to insert the pending callback");
    job_id
}

#[expect(clippy::expect_used, reason = "test helper")]
fn insert_skipped_callback(server: &TestServer, job_uuid: bencher_json::JobUuid) {
    use bencher_schema::model::runner::InsertJobCallback;

    InsertJobCallback::skipped(get_job_id(server, job_uuid), base_timestamp())
        .insert(&mut server.db_conn())
        .expect("Failed to insert the skipped callback");
}

/// Helper: claim a pending callback and record one attempt, as a delivery does.
#[expect(clippy::expect_used, reason = "test helper")]
fn attempt_callback(server: &TestServer, job_id: JobId, status: StatusCode) {
    use bencher_schema::model::runner::QueryJobCallback;

    let mut conn = server.db_conn();
    let now = base_timestamp();
    assert!(
        QueryJobCallback::claim(&mut conn, job_id, now).expect("Failed to claim"),
        "the callback is claimed"
    );
    assert!(
        QueryJobCallback::record_attempt(&mut conn, job_id, Some(status), now)
            .expect("Failed to record the attempt"),
        "the attempt is recorded"
    );
}

#[expect(clippy::expect_used, reason = "test helper")]
fn finish_callback(server: &TestServer, job_id: JobId, outcome: CallbackOutcome) {
    use bencher_schema::model::runner::QueryJobCallback;

    assert!(
        QueryJobCallback::finish(&mut server.db_conn(), job_id, outcome, base_timestamp())
            .expect("Failed to finish"),
        "the callback is finished"
    );
}

/// Helper: GET one job, returning the raw body.
#[expect(clippy::expect_used, reason = "test helper")]
async fn get_job_body(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    job_uuid: bencher_json::JobUuid,
) -> String {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "the job is found");
    resp.text().await.expect("Failed to read response")
}

/// Assert a job's `callback`, both on the wire and as the client parses it.
#[expect(clippy::expect_used, reason = "test helper")]
fn assert_callback(job: &serde_json::Value, expected: Option<serde_json::Value>) {
    assert_eq!(
        job.get("callback"),
        expected.as_ref(),
        "the callback in {job}"
    );
    let parsed: bencher_json::JsonJob =
        serde_json::from_value(job.clone()).expect("Failed to parse the job");
    let expected = expected.map(|callback| {
        serde_json::from_value::<bencher_json::runner::JsonJobCallback>(callback)
            .expect("Failed to parse the expected callback")
    });
    assert_eq!(parsed.callback, expected, "the client parses the callback");
}

// GET /v0/projects/{project}/jobs/{job} - the callback's state, and never its request
#[tokio::test]
async fn job_get_callback() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobcallback@example.com").await;
    let org = server.create_org(&user, "Job Callback Org").await;
    let project = server
        .create_project(&user, &org, "Job Callback Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let now = base_timestamp();
    let project_slug: &str = project.slug.as_ref();

    let none = insert_test_job(&server, report_id, project.uuid, now);
    let pending = insert_test_job(&server, report_id, project.uuid, now);
    insert_pending_callback(&server, pending);
    let delivering = insert_test_job(&server, report_id, project.uuid, now);
    let delivering_id = insert_pending_callback(&server, delivering);
    attempt_callback(&server, delivering_id, StatusCode::BAD_GATEWAY);
    let skipped = insert_test_job(&server, report_id, project.uuid, now);
    insert_skipped_callback(&server, skipped);

    for (job_uuid, expected) in [
        (none, None),
        (
            pending,
            Some(serde_json::json!({ "state": "pending", "status": null })),
        ),
        // A delivery in flight shows as pending, with the status of its last attempt.
        (
            delivering,
            Some(serde_json::json!({ "state": "pending", "status": 502 })),
        ),
        (
            skipped,
            Some(serde_json::json!({ "state": "skipped", "status": null })),
        ),
    ] {
        let body = get_job_body(&server, &user.token, project_slug, job_uuid).await;
        assert!(
            !body.contains(CALLBACK_MARKER),
            "the response holds nothing of the sealed request: {body}"
        );
        let job: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse job");
        assert_eq!(job["uuid"], serde_json::json!(job_uuid));
        assert_callback(&job, expected);
    }
}

// GET /v0/projects/{project}/jobs - each job carries its own callback
#[tokio::test]
async fn jobs_list_callbacks() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobscallback@example.com").await;
    let org = server.create_org(&user, "Jobs Callback Org").await;
    let project = server
        .create_project(&user, &org, "Jobs Callback Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_a = create_test_report(&server, project_id);
    let report_b = create_sibling_report(&server, report_a);
    let now = base_timestamp();

    let delivered = insert_test_job(&server, report_b, project.uuid, now);
    let failed = insert_test_job(&server, report_a, project.uuid, now);
    let none = insert_test_job(&server, report_b, project.uuid, now);
    let pending = insert_test_job(&server, report_a, project.uuid, now);
    let skipped = insert_test_job(&server, report_b, project.uuid, now);

    // Inserted out of job order, so no callback lines up with its job by row order.
    insert_skipped_callback(&server, skipped);
    insert_pending_callback(&server, pending);
    let failed_id = insert_pending_callback(&server, failed);
    attempt_callback(&server, failed_id, StatusCode::INTERNAL_SERVER_ERROR);
    finish_callback(&server, failed_id, CallbackOutcome::Failed);
    let delivered_id = insert_pending_callback(&server, delivered);
    attempt_callback(&server, delivered_id, StatusCode::NO_CONTENT);
    finish_callback(&server, delivered_id, CallbackOutcome::Delivered);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let total_count = resp
        .headers()
        .get("X-Total-Count")
        .expect("X-Total-Count header")
        .to_str()
        .expect("X-Total-Count is text")
        .to_owned();
    assert_eq!(total_count, "5", "the callback join adds no rows");
    let body = resp.text().await.expect("Failed to read response");
    assert!(
        !body.contains(CALLBACK_MARKER),
        "the response holds nothing of a sealed request: {body}"
    );
    let jobs: Vec<serde_json::Value> = serde_json::from_str(&body).expect("Failed to parse jobs");
    assert_eq!(jobs.len(), 5);

    for (job_uuid, expected) in [
        (
            delivered,
            Some(serde_json::json!({ "state": "delivered", "status": 204 })),
        ),
        (
            failed,
            Some(serde_json::json!({ "state": "failed", "status": 500 })),
        ),
        (none, None),
        (
            pending,
            Some(serde_json::json!({ "state": "pending", "status": null })),
        ),
        (
            skipped,
            Some(serde_json::json!({ "state": "skipped", "status": null })),
        ),
    ] {
        let job = jobs
            .iter()
            .find(|job| job["uuid"] == serde_json::json!(job_uuid))
            .expect("the job is listed");
        assert_callback(job, expected);
    }
}
