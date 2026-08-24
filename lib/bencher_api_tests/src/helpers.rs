//! Shared test helpers for direct database manipulation.
//!
//! These helpers are used by both `api_projects` and `api_runners` integration tests.

use bencher_json::{
    BranchUuid, DateTime, HeadUuid, JobStatus, JobUuid, Jwt, MetricName, MetricUuid, ParameterSet,
    ParameterUuid, ReportUuid, ResourceName, TestbedUuid, TokenUuid, VersionUuid,
};
use bencher_schema::{context::DbConnection, model::user::UserId, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

use crate::{TestServer, seed::TestUser};

/// Fixed base timestamp for deterministic tests (Unix epoch + 1 billion seconds).
#[expect(clippy::expect_used, reason = "test helper creating fixed timestamp")]
pub fn base_timestamp() -> DateTime {
    DateTime::try_from(1_000_000_000i64).expect("valid timestamp")
}

/// Get `project_id` from project slug.
#[expect(clippy::expect_used, reason = "test helper querying project ID")]
pub fn get_project_id(server: &TestServer, project_slug: &str) -> i32 {
    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::slug.eq(project_slug))
        .filter(schema::project::deleted.is_null())
        .select(schema::project::id)
        .first(&mut conn)
        .expect("Failed to get project ID")
}

/// Create the empty parameter set that every benchmark is born with.
///
/// Benchmarks inserted directly into the database bypass `QueryBenchmark::create`,
/// so they need the birth invariant applied by hand.
#[expect(clippy::expect_used, reason = "test helper inserting a parameter set")]
pub fn create_empty_parameter(conn: &mut DbConnection, benchmark_id: i32) -> i32 {
    let now = base_timestamp();

    let parameter_uuid = ParameterUuid::new();
    diesel::insert_into(schema::parameter::table)
        .values((
            schema::parameter::uuid.eq(&parameter_uuid),
            schema::parameter::benchmark_id.eq(benchmark_id),
            schema::parameter::set.eq(ParameterSet::default()),
            schema::parameter::created.eq(&now),
            schema::parameter::modified.eq(&now),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert parameter");

    schema::parameter::table
        .filter(schema::parameter::uuid.eq(&parameter_uuid))
        .select(schema::parameter::id)
        .first(&mut *conn)
        .expect("Failed to get parameter ID")
}

/// Get a benchmark's empty parameter set.
#[expect(clippy::expect_used, reason = "test helper querying a parameter set")]
pub fn get_empty_parameter(conn: &mut DbConnection, benchmark_id: i32) -> i32 {
    schema::parameter::table
        .filter(schema::parameter::benchmark_id.eq(benchmark_id))
        .filter(schema::parameter::set.eq(ParameterSet::default()))
        .select(schema::parameter::id)
        .first(&mut *conn)
        .expect("Failed to get empty parameter set")
}

/// Create minimal test infrastructure (testbed, version, branch, head, report).
/// Returns the report ID. Uses a deterministic timestamp.
#[expect(
    clippy::expect_used,
    reason = "test helper inserting test infrastructure"
)]
pub fn create_test_report(server: &TestServer, project_id: i32) -> i32 {
    let mut conn = server.db_conn();
    let now = base_timestamp();

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

/// A user API token seeded directly into the database.
pub struct TestToken {
    pub uuid: TokenUuid,
    pub token: Jwt,
}

/// Seed a user API token directly into the database.
/// The `POST /v0/users/{user}/tokens` endpoint is deprecated and always fails,
/// so tests for the remaining token endpoints must seed tokens at the DB layer.
#[expect(clippy::expect_used, reason = "test helper inserting a user API token")]
pub fn seed_token(server: &TestServer, user: &TestUser, name: &str) -> TestToken {
    let mut conn = server.db_conn();

    let name: ResourceName = name.parse().expect("Invalid token name");
    let user_id: UserId = schema::user::table
        .filter(schema::user::uuid.eq(&user.uuid))
        .select(schema::user::id)
        .first(&mut conn)
        .expect("Failed to get user ID");

    let jwt = server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX)
        .expect("Failed to mint user API token");
    let claims = server
        .token_key()
        .validate_api_key(&jwt)
        .expect("Failed to validate user API token");

    let uuid = TokenUuid::new();
    diesel::insert_into(schema::token::table)
        .values((
            schema::token::uuid.eq(&uuid),
            schema::token::user_id.eq(user_id),
            schema::token::name.eq(&name),
            schema::token::jwt.eq(&jwt),
            schema::token::creation.eq(claims.issued_at()),
            schema::token::expiration.eq(claims.expiration()),
        ))
        .execute(&mut conn)
        .expect("Failed to insert token");

    TestToken { uuid, token: jwt }
}

/// Insert a metric as its named rows.
///
/// The point estimate carries `metric_uuid` and the name `value`; each bound that
/// is present becomes its own row under its conventional name. This is the shape
/// the metric triple takes in storage, so tests seeding metrics by hand write it
/// the way ingest does.
#[expect(clippy::expect_used, reason = "test helper inserting metric rows")]
pub fn create_metric(
    conn: &mut DbConnection,
    metric_uuid: &MetricUuid,
    report_benchmark_id: i32,
    measure_id: i32,
    value: f64,
    lower_value: Option<f64>,
    upper_value: Option<f64>,
) {
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(metric_uuid),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(value),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert metric value");

    for (name, bound) in [
        (MetricName::lower_value(), lower_value),
        (MetricName::upper_value(), upper_value),
    ] {
        let Some(bound) = bound else {
            continue;
        };
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(MetricUuid::new()),
                schema::metric::report_benchmark_id.eq(report_benchmark_id),
                schema::metric::measure_id.eq(measure_id),
                schema::metric::name.eq(name),
                schema::metric::value.eq(bound),
            ))
            .execute(&mut *conn)
            .expect("Failed to insert metric bound");
    }
}

/// Set job status directly in the database.
#[expect(clippy::expect_used, reason = "test helper updating job status")]
pub fn set_job_status(server: &TestServer, job_uuid: JobUuid, status: JobStatus) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::status.eq(status))
        .execute(&mut conn)
        .expect("Failed to set job status");
}
