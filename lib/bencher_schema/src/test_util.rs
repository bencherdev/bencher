//! Test utilities for `bencher_schema` unit tests.
//!
//! This module provides helper functions for setting up test databases
//! and creating test fixtures for unit testing.

use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection,
};

use crate::{macros::sql::last_insert_rowid, run_migrations, schema};

/// Create an in-memory `SQLite` database with migrations applied.
pub fn setup_test_db() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database");
    run_migrations(&mut conn).expect("Failed to run migrations");
    conn
}

/// IDs returned from creating base entities.
#[derive(Debug, Clone, Copy)]
pub struct BaseEntityIds {
    pub organization_id: i32,
    pub project_id: i32,
}

/// Create the base entities (organization, project) required for most tests.
pub fn create_base_entities(conn: &mut SqliteConnection) -> BaseEntityIds {
    diesel::insert_into(schema::organization::table)
        .values((
            schema::organization::uuid.eq("00000000-0000-0000-0000-000000000001"),
            schema::organization::name.eq("Test Org"),
            schema::organization::slug.eq("test-org"),
            schema::organization::created.eq(0i64),
            schema::organization::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert organization");

    diesel::insert_into(schema::project::table)
        .values((
            schema::project::uuid.eq("00000000-0000-0000-0000-000000000002"),
            schema::project::organization_id.eq(1),
            schema::project::name.eq("Test Project"),
            schema::project::slug.eq("test-project"),
            schema::project::visibility.eq(0),
            schema::project::created.eq(0i64),
            schema::project::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert project");

    BaseEntityIds {
        organization_id: 1,
        project_id: 1,
    }
}

/// IDs returned from creating a branch with head.
#[derive(Debug, Clone, Copy)]
pub struct BranchIds {
    pub branch_id: i32,
    pub head_id: i32,
}

/// Create a branch and head for testing.
pub fn create_branch_with_head(
    conn: &mut SqliteConnection,
    project_id: i32,
    branch_uuid: &str,
    branch_name: &str,
    branch_slug: &str,
    head_uuid: &str,
) -> BranchIds {
    // First create branch without head_id
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq(branch_name),
            schema::branch::slug.eq(branch_slug),
            schema::branch::created.eq(0i64),
            schema::branch::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert branch");

    let branch_id: i32 = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get branch id");

    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert head");

    let head_id: i32 = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get head id");

    // Update branch to point to head
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(conn)
        .expect("Failed to update branch head_id");

    BranchIds { branch_id, head_id }
}

/// Create a version for testing.
pub fn create_version(
    conn: &mut SqliteConnection,
    project_id: i32,
    version_uuid: &str,
    version_number: i32,
    hash: Option<&str>,
) -> i32 {
    let values = if let Some(h) = hash {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(version_number),
                schema::version::hash.eq(h),
            ))
            .execute(conn)
    } else {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(version_number),
            ))
            .execute(conn)
    };
    values.expect("Failed to insert version");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get version id")
}

/// Link a version to a head.
pub fn create_head_version(conn: &mut SqliteConnection, head_id: i32, version_id: i32) -> i32 {
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(conn)
        .expect("Failed to insert head_version");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get head_version id")
}

/// Create a testbed for testing.
pub fn create_testbed(
    conn: &mut SqliteConnection,
    project_id: i32,
    testbed_uuid: &str,
    testbed_name: &str,
    testbed_slug: &str,
) -> i32 {
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(testbed_name),
            schema::testbed::slug.eq(testbed_slug),
            schema::testbed::created.eq(0i64),
            schema::testbed::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert testbed");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get testbed id")
}

/// Create a measure for testing.
pub fn create_measure(
    conn: &mut SqliteConnection,
    project_id: i32,
    measure_uuid: &str,
    measure_name: &str,
    measure_slug: &str,
) -> i32 {
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(measure_name),
            schema::measure::slug.eq(measure_slug),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(0i64),
            schema::measure::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert measure");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get measure id")
}

/// Create a threshold for testing.
pub fn create_threshold(
    conn: &mut SqliteConnection,
    project_id: i32,
    branch_id: i32,
    testbed_id: i32,
    measure_id: i32,
    threshold_uuid: &str,
) -> i32 {
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(threshold_uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch_id),
            schema::threshold::testbed_id.eq(testbed_id),
            schema::threshold::measure_id.eq(measure_id),
            schema::threshold::created.eq(0i64),
            schema::threshold::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert threshold");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get threshold id")
}

/// Create a model for a threshold.
pub fn create_model(
    conn: &mut SqliteConnection,
    threshold_id: i32,
    model_uuid: &str,
    test_type: i32,
) -> i32 {
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(test_type),
            schema::model::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert model");

    let model_id: i32 = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get model id");

    // Update threshold to reference model
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(conn)
        .expect("Failed to update threshold with model_id");

    model_id
}

/// Get all `head_version` records for a given head.
pub fn get_head_versions(conn: &mut SqliteConnection, head_id: i32) -> Vec<(i32, i32)> {
    use diesel::QueryDsl as _;

    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .select((
            schema::head_version::head_id,
            schema::head_version::version_id,
        ))
        .load::<(i32, i32)>(conn)
        .expect("Failed to get head_versions")
}

/// Get count of `head_version` records for a given head.
pub fn count_head_versions(conn: &mut SqliteConnection, head_id: i32) -> i64 {
    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .count()
        .get_result(conn)
        .expect("Failed to count head_versions")
}

/// Get all thresholds for a given branch.
pub fn get_thresholds_for_branch(conn: &mut SqliteConnection, branch_id: i32) -> Vec<i32> {
    schema::threshold::table
        .filter(schema::threshold::branch_id.eq(branch_id))
        .select(schema::threshold::id)
        .load::<i32>(conn)
        .expect("Failed to get thresholds")
}

/// Get threshold `model_id`.
pub fn get_threshold_model_id(conn: &mut SqliteConnection, threshold_id: i32) -> Option<i32> {
    schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .select(schema::threshold::model_id)
        .first::<Option<i32>>(conn)
        .expect("Failed to get threshold model_id")
}

/// Arguments for creating a spec.
#[derive(Debug, Clone, Copy)]
pub struct CreateSpecArgs<'a> {
    pub uuid: &'a str,
    pub name: &'a str,
    pub slug: &'a str,
    pub architecture: &'a str,
    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,
    pub network: bool,
}

/// Create a spec for testing.
pub fn create_spec(conn: &mut SqliteConnection, args: CreateSpecArgs<'_>) -> i32 {
    let CreateSpecArgs {
        uuid,
        name,
        slug,
        architecture,
        cpu,
        memory,
        disk,
        network,
    } = args;
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(uuid),
            schema::spec::name.eq(name),
            schema::spec::slug.eq(slug),
            schema::spec::architecture.eq(architecture),
            schema::spec::cpu.eq(cpu),
            schema::spec::memory.eq(memory),
            schema::spec::disk.eq(disk),
            schema::spec::network.eq(network),
            schema::spec::created.eq(0i64),
            schema::spec::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert spec");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get spec id")
}

/// Get testbed `spec_id`.
pub fn get_testbed_spec_id(conn: &mut SqliteConnection, testbed_id: i32) -> Option<i32> {
    schema::testbed::table
        .filter(schema::testbed::id.eq(testbed_id))
        .select(schema::testbed::spec_id)
        .first::<Option<i32>>(conn)
        .expect("Failed to get testbed spec_id")
}

/// Set testbed `spec_id`.
pub fn set_testbed_spec(conn: &mut SqliteConnection, testbed_id: i32, spec_id: i32) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::spec_id.eq(Some(spec_id)))
        .execute(conn)
        .expect("Failed to set testbed spec_id");
}

/// Clear testbed `spec_id`.
pub fn clear_testbed_spec(conn: &mut SqliteConnection, testbed_id: i32) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::spec_id.eq(None::<i32>))
        .execute(conn)
        .expect("Failed to clear testbed spec_id");
}

/// Delete a spec by id.
pub fn delete_spec(conn: &mut SqliteConnection, spec_id: i32) {
    diesel::delete(schema::spec::table.filter(schema::spec::id.eq(spec_id)))
        .execute(conn)
        .expect("Failed to delete spec");
}

/// Archive a testbed.
pub fn archive_testbed(conn: &mut SqliteConnection, testbed_id: i32) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::archived.eq(Some(1i64)))
        .execute(conn)
        .expect("Failed to archive testbed");
}

/// Get testbed `archived` timestamp.
pub fn get_testbed_archived(conn: &mut SqliteConnection, testbed_id: i32) -> Option<i64> {
    schema::testbed::table
        .filter(schema::testbed::id.eq(testbed_id))
        .select(schema::testbed::archived)
        .first::<Option<i64>>(conn)
        .expect("Failed to get testbed archived")
}

/// Create a report for testing.
pub fn create_report(
    conn: &mut SqliteConnection,
    report_uuid: &str,
    project_id: i32,
    head_id: i32,
    version_id: i32,
    testbed_id: i32,
) -> i32 {
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(report_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(head_id),
            schema::report::version_id.eq(version_id),
            schema::report::testbed_id.eq(testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(0i64),
            schema::report::end_time.eq(0i64),
            schema::report::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert report");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get report id")
}

/// Create a benchmark for testing.
pub fn create_benchmark(
    conn: &mut SqliteConnection,
    project_id: i32,
    benchmark_uuid: &str,
    benchmark_name: &str,
    benchmark_slug: &str,
) -> i32 {
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(benchmark_name),
            schema::benchmark::slug.eq(benchmark_slug),
            schema::benchmark::created.eq(0i64),
            schema::benchmark::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert benchmark");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get benchmark id")
}

/// Create a report benchmark for testing.
pub fn create_report_benchmark(
    conn: &mut SqliteConnection,
    report_benchmark_uuid: &str,
    report_id: i32,
    iteration: i32,
    benchmark_id: i32,
) -> i32 {
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(report_benchmark_uuid),
            schema::report_benchmark::report_id.eq(report_id),
            schema::report_benchmark::iteration.eq(iteration),
            schema::report_benchmark::benchmark_id.eq(benchmark_id),
        ))
        .execute(conn)
        .expect("Failed to insert report_benchmark");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get report_benchmark id")
}

/// Create a metric for testing.
pub fn create_metric(
    conn: &mut SqliteConnection,
    metric_uuid: &str,
    report_benchmark_id: i32,
    measure_id: i32,
    value: f64,
) -> i32 {
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(metric_uuid),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(measure_id),
            schema::metric::value.eq(value),
        ))
        .execute(conn)
        .expect("Failed to insert metric");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get metric id")
}

/// Create a boundary for testing.
pub fn create_boundary(
    conn: &mut SqliteConnection,
    boundary_uuid: &str,
    metric_id: i32,
    threshold_id: i32,
    model_id: i32,
) -> i32 {
    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(boundary_uuid),
            schema::boundary::metric_id.eq(metric_id),
            schema::boundary::threshold_id.eq(threshold_id),
            schema::boundary::model_id.eq(model_id),
        ))
        .execute(conn)
        .expect("Failed to insert boundary");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get boundary id")
}

/// Create an alert for testing.
pub fn create_alert(
    conn: &mut SqliteConnection,
    alert_uuid: &str,
    boundary_id: i32,
    boundary_limit: bool,
    status: i32,
) -> i32 {
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(alert_uuid),
            schema::alert::boundary_id.eq(boundary_id),
            schema::alert::boundary_limit.eq(boundary_limit),
            schema::alert::status.eq(status),
            schema::alert::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert alert");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get alert id")
}

/// Get alert status by id.
pub fn get_alert_status(conn: &mut SqliteConnection, alert_id: i32) -> i32 {
    schema::alert::table
        .filter(schema::alert::id.eq(alert_id))
        .select(schema::alert::status)
        .first::<i32>(conn)
        .expect("Failed to get alert status")
}

/// Count alerts through the report JOIN chain for a given `head_id`.
pub fn count_alerts_for_head(conn: &mut SqliteConnection, head_id: i32) -> i64 {
    schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table
                    .inner_join(schema::report_benchmark::table.inner_join(schema::report::table)),
            ),
        )
        .filter(schema::report::head_id.eq(head_id))
        .count()
        .get_result(conn)
        .expect("Failed to count alerts for head")
}
