// Each test file (`jobs.rs`, `channel.rs`, etc.) includes this module separately,
// so not all helpers are used by every test binary.
#![allow(dead_code, unused_imports)]
//! Shared test helpers for `api_runners` integration tests.
//!
//! Common helpers (`get_project_id`, `create_test_report`, `set_job_status`,
//! `base_timestamp`) are re-exported from `bencher_api_tests::helpers`.
//! Runner-specific helpers live here.

use bencher_api_tests::TestServer;
pub use bencher_api_tests::helpers::{
    base_timestamp, create_test_report, get_project_id, set_job_status,
};
use bencher_json::{DateTime, JobPriority, JobStatus, JobUuid, JsonRunnerToken, SpecUuid};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

/// Create a runner via the REST API.
#[expect(clippy::expect_used)]
pub async fn create_runner(server: &TestServer, admin_token: &str, name: &str) -> JsonRunnerToken {
    let body = serde_json::json!({ "name": name });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(admin_token),
        )
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

/// Insert a test spec directly into the database. Returns the spec UUID and `spec_id`.
pub fn insert_test_spec(server: &TestServer) -> (SpecUuid, i32) {
    insert_test_spec_full(server, "x86_64", 2, 0x0001_0000_0000, 10_737_418_240, false)
}

/// Insert a test spec with specific values. Returns (`SpecUuid`, `spec_id`).
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
    let now = base_timestamp();
    let spec_uuid = SpecUuid::new();
    let name = format!("test-spec-{spec_uuid}");
    let slug = format!("test-spec-{spec_uuid}");

    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::name.eq(&name),
            schema::spec::slug.eq(&slug),
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
/// Looks up the `organization_id` from the project associated with the report.
pub fn insert_test_job(server: &TestServer, report_id: i32, spec_id: i32) -> JobUuid {
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);
    insert_test_job_full(
        server,
        report_id,
        bencher_json::ProjectUuid::new(),
        organization_id,
        TEST_SOURCE_IP,
        JobPriority::default(),
        spec_id,
    )
}

/// Insert a test job with a specific project UUID. Returns the job UUID.
pub fn insert_test_job_with_project(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
) -> JobUuid {
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);
    insert_test_job_full(
        server,
        report_id,
        project_uuid,
        organization_id,
        TEST_SOURCE_IP,
        JobPriority::default(),
        spec_id,
    )
}

/// Insert a test job with a custom timeout (in seconds). Returns the job UUID.
#[expect(clippy::expect_used)]
pub fn insert_test_job_with_timeout(
    server: &TestServer,
    report_id: i32,
    spec_id: i32,
    timeout_secs: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_uuid = bencher_json::ProjectUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": timeout_secs
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(timeout_secs),
            schema::job::priority.eq(JobPriority::default()),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
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
    let now = base_timestamp();
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

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
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
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

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
            schema::job::organization_id.eq(organization_id),
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

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

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
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    // Invalid config - missing required fields like digest, timeout, etc.
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev"
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
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

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Get project ID from report ID.
#[expect(clippy::expect_used)]
pub fn get_project_id_from_report(server: &TestServer, report_id: i32) -> i32 {
    let mut conn = server.db_conn();
    schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select(schema::report::project_id)
        .first(&mut conn)
        .expect("Failed to get project ID from report")
}

/// Get organization ID from project ID.
pub fn get_organization_id(server: &TestServer, project_id: i32) -> i32 {
    get_organization_id_from_project_id(server, project_id)
}

/// Get organization ID from project ID (by primary key).
#[expect(clippy::expect_used)]
pub fn get_organization_id_from_project_id(server: &TestServer, project_id: i32) -> i32 {
    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(&mut conn)
        .expect("Failed to get organization ID")
}

/// Set the `runner_id` directly in the database (for testing preconditions).
#[expect(clippy::expect_used)]
pub fn set_job_runner_id(server: &TestServer, job_uuid: JobUuid, runner_id: i32) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::runner_id.eq(Some(runner_id)))
        .execute(&mut conn)
        .expect("Failed to set job runner_id");
}

/// Insert a test job with a specific created timestamp (for FIFO tiebreaker tests).
#[expect(clippy::too_many_arguments, clippy::expect_used)]
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

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Get the priority of a job directly from the database.
#[expect(clippy::expect_used)]
pub fn get_job_priority(server: &TestServer, job_uuid: JobUuid) -> JobPriority {
    let mut conn = server.db_conn();
    schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::priority)
        .first(&mut conn)
        .expect("Failed to get job priority")
}

/// Get `runner_id` (as i32) from runner UUID.
#[expect(clippy::expect_used)]
pub fn get_runner_id(server: &TestServer, runner_uuid: bencher_json::RunnerUuid) -> i32 {
    let mut conn = server.db_conn();
    schema::runner::table
        .filter(schema::runner::uuid.eq(runner_uuid))
        .select(schema::runner::id)
        .first(&mut conn)
        .expect("Failed to get runner ID")
}
