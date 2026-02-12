// Each test file (`jobs.rs`, `channel.rs`, etc.) includes this module separately,
// so not all helpers are used by every test binary.
#![allow(dead_code)]
//! Shared test helpers for `api_runners` integration tests.
//!
//! Note: Similar helpers exist in `lib/api_projects/tests/jobs.rs`.
//! They are kept separate because the two test suites live in different
//! crates and have slightly different signatures. Both copies use UUID-based
//! slugs to avoid UNIQUE constraint collisions across tests.

use bencher_api_tests::TestServer;
use bencher_json::{
    BranchUuid, DateTime, JobPriority, JobStatus, JobUuid, JsonRunnerToken, SpecUuid,
};
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
    assert_eq!(
        resp.status(),
        http::StatusCode::CREATED,
        "Failed to create runner"
    );
    resp.json().await.expect("Failed to parse response")
}

/// Default test source IP for job insertion.
pub const TEST_SOURCE_IP: &str = "127.0.0.1";

/// Insert a test spec directly into the database. Returns the spec UUID and spec_id.
#[expect(clippy::expect_used)]
pub fn insert_test_spec(server: &TestServer) -> (SpecUuid, i32) {
    insert_test_spec_full(server, "x86_64", 2, 4_294_967_296, 10_737_418_240, false)
}

/// Insert a test spec with specific values. Returns (SpecUuid, spec_id).
#[expect(clippy::expect_used)]
pub fn insert_test_spec_full(
    server: &TestServer,
    architecture: &str,
    cpu: i32,
    memory: i64,
    disk: i64,
    network: bool,
) -> (SpecUuid, i32) {
    let mut conn = server.db_conn();
    let now = DateTime::now();
    let spec_uuid = SpecUuid::new();

    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::architecture.eq(architecture),
            schema::spec::cpu.eq(cpu),
            schema::spec::memory.eq(memory),
            schema::spec::disk.eq(disk),
            schema::spec::network.eq(network),
            schema::spec::created.eq(&now),
            schema::spec::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test spec");

    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec_uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to get spec ID");

    (spec_uuid, spec_id)
}

/// Associate a spec with a runner.
#[expect(clippy::expect_used)]
pub fn associate_runner_spec(server: &TestServer, runner_id: i32, spec_id: i32) {
    let mut conn = server.db_conn();
    diesel::insert_into(schema::runner_spec::table)
        .values((
            schema::runner_spec::runner_id.eq(runner_id),
            schema::runner_spec::spec_id.eq(spec_id),
        ))
        .execute(&mut conn)
        .expect("Failed to associate runner with spec");
}

/// Insert a test job directly into the database. Returns the job UUID.
/// Uses a default organization_id of 1 and source_ip of "127.0.0.1".
#[expect(clippy::expect_used)]
pub fn insert_test_job(server: &TestServer, report_id: i32, spec_id: i32) -> JobUuid {
    insert_test_job_full(
        server,
        report_id,
        bencher_json::ProjectUuid::new(),
        1,
        TEST_SOURCE_IP,
        JobPriority::default(),
        spec_id,
    )
}

/// Insert a test job with a specific project UUID. Returns the job UUID.
#[expect(clippy::expect_used)]
pub fn insert_test_job_with_project(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
) -> JobUuid {
    insert_test_job_full(
        server,
        report_id,
        project_uuid,
        1,
        TEST_SOURCE_IP,
        JobPriority::default(),
        spec_id,
    )
}

/// Insert a test job with full control over scheduling parameters.
#[expect(clippy::expect_used)]
pub fn insert_test_job_full(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    organization_id: i32,
    source_ip: &str,
    priority: JobPriority,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = DateTime::now();
    let job_uuid = JobUuid::new();

    // Create a valid JsonJobConfig as JSON
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
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(source_ip),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(priority),
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

/// Insert a test job with optional fields populated. Returns the job UUID.
#[expect(clippy::expect_used)]
pub fn insert_test_job_with_optional_fields(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = DateTime::now();
    let job_uuid = JobUuid::new();

    // Create a JsonJobConfig with optional fields populated
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "entrypoint": ["/bin/sh", "-c"],
        "cmd": ["cargo", "bench"],
        "env": {
            "RUST_LOG": "info",
            "CI": "true"
        },
        "timeout": 7200,
        "file_paths": ["/output/results.json", "/tmp/bench.txt"]
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(1),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(7200),
            schema::job::priority.eq(JobPriority::default()),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    job_uuid
}

/// Insert a test job with invalid config JSON (missing required fields). Returns the job UUID.
#[expect(clippy::expect_used)]
pub fn insert_test_job_with_invalid_config(
    server: &TestServer,
    report_id: i32,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = DateTime::now();
    let job_uuid = JobUuid::new();

    // Invalid config - missing required fields like digest, timeout, etc.
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev"
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(1),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(JobPriority::default()),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    job_uuid
}

/// Get organization ID from project ID.
#[expect(clippy::expect_used)]
pub fn get_organization_id(server: &TestServer, project_id: i32) -> i32 {
    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(&mut conn)
        .expect("Failed to get organization ID")
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

/// Set the job status directly in the database (for testing state transitions).
#[expect(clippy::expect_used)]
pub fn set_job_status(server: &TestServer, job_uuid: JobUuid, status: JobStatus) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::status.eq(status))
        .execute(&mut conn)
        .expect("Failed to set job status");
}

/// Set the runner_id directly in the database (for testing preconditions).
#[expect(clippy::expect_used)]
pub fn set_job_runner_id(server: &TestServer, job_uuid: JobUuid, runner_id: i32) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::runner_id.eq(Some(runner_id)))
        .execute(&mut conn)
        .expect("Failed to set job runner_id");
}

/// Insert a test job with a specific created timestamp (for FIFO tiebreaker tests).
#[expect(clippy::expect_used)]
pub fn insert_test_job_with_timestamp(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    organization_id: i32,
    source_ip: &str,
    priority: JobPriority,
    created: DateTime,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let job_uuid = JobUuid::new();

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
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(source_ip),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(priority),
            schema::job::created.eq(&created),
            schema::job::modified.eq(&created),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    job_uuid
}

/// Get runner_id (as i32) from runner UUID.
#[expect(clippy::expect_used)]
pub fn get_runner_id(server: &TestServer, runner_uuid: bencher_json::RunnerUuid) -> i32 {
    let mut conn = server.db_conn();
    schema::runner::table
        .filter(schema::runner::uuid.eq(runner_uuid))
        .select(schema::runner::id)
        .first(&mut conn)
        .expect("Failed to get runner ID")
}
