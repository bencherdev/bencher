//! Test utilities for `bencher_schema` unit tests.
//!
//! This module provides helper functions for setting up test databases
//! and creating test fixtures for unit testing.
//!
//! # Transaction safety of `last_insert_rowid()`
//!
//! The helpers in this module call `last_insert_rowid()` outside explicit transactions.
//! This is safe because each test runs on its own single-threaded, in-memory `SqliteConnection`,
//! so no concurrent INSERT can interleave between the INSERT and the `last_insert_rowid()` call.

use bencher_json::{
    DateTime,
    project::{alert::AlertStatus, boundary::BoundaryLimit},
};
use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection,
};

use crate::{
    macros::sql::last_insert_rowid,
    model::{
        organization::OrganizationId,
        project::{
            ProjectId,
            benchmark::BenchmarkId,
            branch::{BranchId, head::HeadId, head_version::HeadVersionId, version::VersionId},
            measure::MeasureId,
            metric::MetricId,
            plot::PlotId,
            report::{ReportId, report_benchmark::ReportBenchmarkId},
            testbed::TestbedId,
            threshold::{ThresholdId, alert::AlertId, boundary::BoundaryId, model::ModelId},
        },
        spec::SpecId,
    },
    run_migrations, schema,
};

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
    pub organization_id: OrganizationId,
    pub project_id: ProjectId,
}

/// Create the base entities (organization, project) required for most tests.
pub fn create_base_entities(conn: &mut SqliteConnection) -> BaseEntityIds {
    diesel::insert_into(schema::organization::table)
        .values((
            schema::organization::uuid.eq("00000000-0000-0000-0000-000000000001"),
            schema::organization::name.eq("Test Org"),
            schema::organization::slug.eq("test-org"),
            schema::organization::created.eq(DateTime::TEST),
            schema::organization::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert organization");

    let organization_id: OrganizationId = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get organization id");

    diesel::insert_into(schema::project::table)
        .values((
            schema::project::uuid.eq("00000000-0000-0000-0000-000000000002"),
            schema::project::organization_id.eq(organization_id),
            schema::project::name.eq("Test Project"),
            schema::project::slug.eq("test-project"),
            schema::project::visibility.eq(0),
            schema::project::created.eq(DateTime::TEST),
            schema::project::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert project");

    let project_id: ProjectId = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get project id");

    BaseEntityIds {
        organization_id,
        project_id,
    }
}

/// IDs returned from creating a branch with head.
#[derive(Debug, Clone, Copy)]
pub struct BranchIds {
    pub branch_id: BranchId,
    pub head_id: HeadId,
}

/// Create a branch and head for testing.
pub fn create_branch_with_head(
    conn: &mut SqliteConnection,
    project_id: ProjectId,
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
            schema::branch::created.eq(DateTime::TEST),
            schema::branch::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert branch");

    let branch_id: BranchId = diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get branch id");

    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert head");

    let head_id: HeadId = diesel::select(last_insert_rowid())
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
    project_id: ProjectId,
    version_uuid: &str,
    version_number: i32,
    hash: Option<&str>,
) -> VersionId {
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
pub fn create_head_version(
    conn: &mut SqliteConnection,
    head_id: HeadId,
    version_id: VersionId,
) -> HeadVersionId {
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
    project_id: ProjectId,
    testbed_uuid: &str,
    testbed_name: &str,
    testbed_slug: &str,
) -> TestbedId {
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(testbed_name),
            schema::testbed::slug.eq(testbed_slug),
            schema::testbed::created.eq(DateTime::TEST),
            schema::testbed::modified.eq(DateTime::TEST),
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
    project_id: ProjectId,
    measure_uuid: &str,
    measure_name: &str,
    measure_slug: &str,
) -> MeasureId {
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(measure_name),
            schema::measure::slug.eq(measure_slug),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(DateTime::TEST),
            schema::measure::modified.eq(DateTime::TEST),
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
    project_id: ProjectId,
    branch_id: BranchId,
    testbed_id: TestbedId,
    measure_id: MeasureId,
    threshold_uuid: &str,
) -> ThresholdId {
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(threshold_uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch_id),
            schema::threshold::testbed_id.eq(testbed_id),
            schema::threshold::measure_id.eq(measure_id),
            schema::threshold::created.eq(DateTime::TEST),
            schema::threshold::modified.eq(DateTime::TEST),
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
    threshold_id: ThresholdId,
    model_uuid: &str,
    test_type: i32,
) -> ModelId {
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(test_type),
            schema::model::created.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert model");

    let model_id: ModelId = diesel::select(last_insert_rowid())
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
pub fn get_head_versions(conn: &mut SqliteConnection, head_id: HeadId) -> Vec<(HeadId, VersionId)> {
    use diesel::QueryDsl as _;

    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .select((
            schema::head_version::head_id,
            schema::head_version::version_id,
        ))
        .load::<(HeadId, VersionId)>(conn)
        .expect("Failed to get head_versions")
}

/// Get count of `head_version` records for a given head.
pub fn count_head_versions(conn: &mut SqliteConnection, head_id: HeadId) -> i64 {
    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .count()
        .get_result(conn)
        .expect("Failed to count head_versions")
}

/// Get all thresholds for a given branch.
pub fn get_thresholds_for_branch(
    conn: &mut SqliteConnection,
    branch_id: BranchId,
) -> Vec<ThresholdId> {
    schema::threshold::table
        .filter(schema::threshold::branch_id.eq(branch_id))
        .select(schema::threshold::id)
        .load::<ThresholdId>(conn)
        .expect("Failed to get thresholds")
}

/// Get threshold `model_id`.
pub fn get_threshold_model_id(
    conn: &mut SqliteConnection,
    threshold_id: ThresholdId,
) -> Option<ModelId> {
    schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .select(schema::threshold::model_id)
        .first::<Option<ModelId>>(conn)
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
pub fn create_spec(conn: &mut SqliteConnection, args: CreateSpecArgs<'_>) -> SpecId {
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
            schema::spec::created.eq(DateTime::TEST),
            schema::spec::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert spec");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get spec id")
}

/// Get testbed `spec_id`.
pub fn get_testbed_spec_id(conn: &mut SqliteConnection, testbed_id: TestbedId) -> Option<SpecId> {
    schema::testbed::table
        .filter(schema::testbed::id.eq(testbed_id))
        .select(schema::testbed::spec_id)
        .first::<Option<SpecId>>(conn)
        .expect("Failed to get testbed spec_id")
}

/// Set testbed `spec_id`.
pub fn set_testbed_spec(conn: &mut SqliteConnection, testbed_id: TestbedId, spec_id: SpecId) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::spec_id.eq(Some(spec_id)))
        .execute(conn)
        .expect("Failed to set testbed spec_id");
}

/// Clear testbed `spec_id`.
pub fn clear_testbed_spec(conn: &mut SqliteConnection, testbed_id: TestbedId) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::spec_id.eq(None::<SpecId>))
        .execute(conn)
        .expect("Failed to clear testbed spec_id");
}

/// Delete a spec by id.
pub fn delete_spec(conn: &mut SqliteConnection, spec_id: SpecId) {
    diesel::delete(schema::spec::table.filter(schema::spec::id.eq(spec_id)))
        .execute(conn)
        .expect("Failed to delete spec");
}

/// Archive a testbed.
pub fn archive_testbed(conn: &mut SqliteConnection, testbed_id: TestbedId) {
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
        .set(schema::testbed::archived.eq(Some(DateTime::TEST)))
        .execute(conn)
        .expect("Failed to archive testbed");
}

/// Get testbed `archived` timestamp.
pub fn get_testbed_archived(
    conn: &mut SqliteConnection,
    testbed_id: TestbedId,
) -> Option<DateTime> {
    schema::testbed::table
        .filter(schema::testbed::id.eq(testbed_id))
        .select(schema::testbed::archived)
        .first::<Option<DateTime>>(conn)
        .expect("Failed to get testbed archived")
}

/// Create a report for testing.
pub fn create_report(
    conn: &mut SqliteConnection,
    report_uuid: &str,
    project_id: ProjectId,
    head_id: HeadId,
    version_id: VersionId,
    testbed_id: TestbedId,
) -> ReportId {
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(report_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(head_id),
            schema::report::version_id.eq(version_id),
            schema::report::testbed_id.eq(testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(DateTime::TEST),
            schema::report::end_time.eq(DateTime::TEST),
            schema::report::created.eq(DateTime::TEST),
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
    project_id: ProjectId,
    benchmark_uuid: &str,
    benchmark_name: &str,
    benchmark_slug: &str,
) -> BenchmarkId {
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(benchmark_name),
            schema::benchmark::slug.eq(benchmark_slug),
            schema::benchmark::created.eq(DateTime::TEST),
            schema::benchmark::modified.eq(DateTime::TEST),
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
    report_id: ReportId,
    iteration: i32,
    benchmark_id: BenchmarkId,
) -> ReportBenchmarkId {
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
    report_benchmark_id: ReportBenchmarkId,
    measure_id: MeasureId,
    value: f64,
) -> MetricId {
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
    metric_id: MetricId,
    threshold_id: ThresholdId,
    model_id: ModelId,
) -> BoundaryId {
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
    boundary_id: BoundaryId,
    boundary_limit: BoundaryLimit,
    status: AlertStatus,
) -> AlertId {
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(alert_uuid),
            schema::alert::boundary_id.eq(boundary_id),
            schema::alert::boundary_limit.eq(boundary_limit),
            schema::alert::status.eq(status),
            schema::alert::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert alert");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get alert id")
}

/// Get alert status by id.
pub fn get_alert_status(conn: &mut SqliteConnection, alert_id: AlertId) -> AlertStatus {
    schema::alert::table
        .filter(schema::alert::id.eq(alert_id))
        .select(schema::alert::status)
        .first::<AlertStatus>(conn)
        .expect("Failed to get alert status")
}

/// Get branch `head_id`.
pub fn get_branch_head_id(conn: &mut SqliteConnection, branch_id: BranchId) -> Option<HeadId> {
    schema::branch::table
        .filter(schema::branch::id.eq(branch_id))
        .select(schema::branch::head_id)
        .first::<Option<HeadId>>(conn)
        .expect("Failed to get branch head_id")
}

/// Get head `replaced` timestamp.
pub fn get_head_replaced(conn: &mut SqliteConnection, head_id: HeadId) -> Option<DateTime> {
    schema::head::table
        .filter(schema::head::id.eq(head_id))
        .select(schema::head::replaced)
        .first::<Option<DateTime>>(conn)
        .expect("Failed to get head replaced")
}

/// Archive a benchmark.
pub fn archive_benchmark(conn: &mut SqliteConnection, benchmark_id: BenchmarkId) {
    diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(benchmark_id)))
        .set(schema::benchmark::archived.eq(Some(DateTime::TEST)))
        .execute(conn)
        .expect("Failed to archive benchmark");
}

/// Archive a measure.
pub fn archive_measure(conn: &mut SqliteConnection, measure_id: MeasureId) {
    diesel::update(schema::measure::table.filter(schema::measure::id.eq(measure_id)))
        .set(schema::measure::archived.eq(Some(DateTime::TEST)))
        .execute(conn)
        .expect("Failed to archive measure");
}

/// Get benchmark `archived` timestamp.
pub fn get_benchmark_archived(
    conn: &mut SqliteConnection,
    benchmark_id: BenchmarkId,
) -> Option<DateTime> {
    schema::benchmark::table
        .filter(schema::benchmark::id.eq(benchmark_id))
        .select(schema::benchmark::archived)
        .first::<Option<DateTime>>(conn)
        .expect("Failed to get benchmark archived")
}

/// Get measure `archived` timestamp.
pub fn get_measure_archived(
    conn: &mut SqliteConnection,
    measure_id: MeasureId,
) -> Option<DateTime> {
    schema::measure::table
        .filter(schema::measure::id.eq(measure_id))
        .select(schema::measure::archived)
        .first::<Option<DateTime>>(conn)
        .expect("Failed to get measure archived")
}

/// Create a plot for testing.
pub fn create_plot(
    conn: &mut SqliteConnection,
    project_id: ProjectId,
    uuid: &str,
    rank: i64,
) -> PlotId {
    diesel::insert_into(schema::plot::table)
        .values((
            schema::plot::uuid.eq(uuid),
            schema::plot::project_id.eq(project_id),
            schema::plot::rank.eq(rank),
            schema::plot::lower_value.eq(true),
            schema::plot::upper_value.eq(true),
            schema::plot::lower_boundary.eq(false),
            schema::plot::upper_boundary.eq(false),
            schema::plot::x_axis.eq(0),
            schema::plot::window.eq(2_592_000i64),
            schema::plot::created.eq(DateTime::TEST),
            schema::plot::modified.eq(DateTime::TEST),
        ))
        .execute(conn)
        .expect("Failed to insert plot");

    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get plot id")
}

/// Get plot rank.
pub fn get_plot_rank(conn: &mut SqliteConnection, plot_id: PlotId) -> i64 {
    schema::plot::table
        .filter(schema::plot::id.eq(plot_id))
        .select(schema::plot::rank)
        .first::<i64>(conn)
        .expect("Failed to get plot rank")
}

/// Get plot branch IDs.
pub fn get_plot_branches(conn: &mut SqliteConnection, plot_id: PlotId) -> Vec<BranchId> {
    schema::plot_branch::table
        .filter(schema::plot_branch::plot_id.eq(plot_id))
        .order(schema::plot_branch::rank.asc())
        .select(schema::plot_branch::branch_id)
        .load(conn)
        .expect("Failed to get plot branches")
}

/// Get plot testbed IDs.
pub fn get_plot_testbeds(conn: &mut SqliteConnection, plot_id: PlotId) -> Vec<TestbedId> {
    schema::plot_testbed::table
        .filter(schema::plot_testbed::plot_id.eq(plot_id))
        .order(schema::plot_testbed::rank.asc())
        .select(schema::plot_testbed::testbed_id)
        .load(conn)
        .expect("Failed to get plot testbeds")
}

/// Get plot benchmark IDs.
pub fn get_plot_benchmarks(conn: &mut SqliteConnection, plot_id: PlotId) -> Vec<BenchmarkId> {
    schema::plot_benchmark::table
        .filter(schema::plot_benchmark::plot_id.eq(plot_id))
        .order(schema::plot_benchmark::rank.asc())
        .select(schema::plot_benchmark::benchmark_id)
        .load(conn)
        .expect("Failed to get plot benchmarks")
}

/// Get plot measure IDs.
pub fn get_plot_measures(conn: &mut SqliteConnection, plot_id: PlotId) -> Vec<MeasureId> {
    schema::plot_measure::table
        .filter(schema::plot_measure::plot_id.eq(plot_id))
        .order(schema::plot_measure::rank.asc())
        .select(schema::plot_measure::measure_id)
        .load(conn)
        .expect("Failed to get plot measures")
}
