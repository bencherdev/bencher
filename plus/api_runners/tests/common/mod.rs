//! Shared test helpers for `api_runners` integration tests.

use bencher_api_tests::TestServer;
use bencher_json::{BranchUuid, DateTime, JobStatus, JobUuid, JsonRunnerToken};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

/// Create a runner via the REST API.
#[expect(clippy::expect_used)]
pub async fn create_runner(server: &TestServer, admin_token: &str, name: &str) -> JsonRunnerToken {
    let body = serde_json::json!({ "name": name });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    resp.json().await.expect("Failed to parse response")
}

/// Insert a test job directly into the database. Returns the job UUID.
#[expect(clippy::expect_used)]
pub fn insert_test_job(server: &TestServer, report_id: i32) -> JobUuid {
    let mut conn = server.db_conn();
    let now = DateTime::now();
    let job_uuid = JobUuid::new();

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec.eq("{}"),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(0),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    job_uuid
}

/// Create minimal test infrastructure (testbed, version, branch, head, report).
/// Returns the report ID.
#[expect(clippy::expect_used)]
pub fn create_test_report(server: &TestServer, project_id: i32) -> i32 {
    let mut conn = server.db_conn();
    let now = DateTime::now();

    let testbed_uuid = bencher_json::TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq("test-testbed"),
            schema::testbed::slug.eq("test-testbed"),
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

    let version_uuid = bencher_json::VersionUuid::new();
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
            schema::branch::slug.eq("main"),
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

    let head_uuid = bencher_json::HeadUuid::new();
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

    let report_uuid = bencher_json::ReportUuid::new();
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

/// Get project ID from slug.
#[expect(clippy::expect_used)]
pub fn get_project_id(server: &TestServer, project_slug: &str) -> i32 {
    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::slug.eq(project_slug))
        .select(schema::project::id)
        .first(&mut conn)
        .expect("Failed to get project ID")
}
