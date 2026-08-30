#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Integration tests for the `/v0/projects/{project}/perf` endpoint.

use std::collections::BTreeMap;

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, create_empty_parameter, create_metric, get_project_id},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, HeadUuid, JobStatus, JobUuid, JsonPerf,
    JsonPerfQuery, MeasureUuid, MetricName, MetricUuid, ParameterSet, ParameterUuid, Priority,
    ReportBenchmarkUuid, ReportUuid, SpecUuid, TestbedUuid, VersionUuid,
    project::{alert::AlertStatus, boundary::BoundaryLimit},
};
use bencher_schema::{
    model::project::report::{ReportId, upsert_metric_count},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

// =============================================================================
// Helper: perf_server
// =============================================================================

/// A test server whose clock is frozen beside the fixtures, which all report at
/// `base_timestamp()` and would otherwise fall outside the default window.
async fn perf_server() -> TestServer {
    TestServer::new_at(base_timestamp()).await
}

// =============================================================================
// Helper: PerfTestData
// =============================================================================

/// Holds all IDs/UUIDs created by `create_perf_data` for assertions and URL construction.
struct PerfTestData {
    branch_uuid: BranchUuid,
    head_uuid: HeadUuid,
    testbed_uuid: TestbedUuid,
    benchmark_uuid: BenchmarkUuid,
    measure_uuid: MeasureUuid,
    report_uuid: ReportUuid,
    metric_uuid: MetricUuid,
    // Internal IDs for further DB manipulation
    branch_id: i32,
    head_id: i32,
    testbed_id: i32,
    benchmark_id: i32,
    parameter_id: i32,
    measure_id: i32,
    report_id: i32,
    report_benchmark_id: i32,
}

/// Options for `create_perf_data_with_options`.
struct PerfDataOptions {
    version_number: i32,
    version_hash: Option<String>,
    metric_value: f64,
    lower_value: Option<f64>,
    upper_value: Option<f64>,
    iteration: i32,
    start_time: bencher_json::DateTime,
    end_time: bencher_json::DateTime,
}

impl Default for PerfDataOptions {
    fn default() -> Self {
        let ts = base_timestamp();
        Self {
            version_number: 1,
            version_hash: None,
            metric_value: 42.0,
            lower_value: None,
            upper_value: None,
            iteration: 0,
            start_time: ts,
            end_time: ts,
        }
    }
}

// =============================================================================
// Helper: create_perf_data / create_perf_data_with_options
// =============================================================================

/// Create the FULL data chain needed for the perf endpoint to return results.
/// Unlike `create_test_report`, this also sets `branch.head_id` and creates `head_version`.
fn create_perf_data(server: &TestServer, project_id: i32) -> PerfTestData {
    create_perf_data_with_options(server, project_id, &PerfDataOptions::default())
}

fn create_perf_data_with_options(
    server: &TestServer,
    project_id: i32,
    opts: &PerfDataOptions,
) -> PerfTestData {
    let mut conn = server.db_conn();
    let now = base_timestamp();

    // Testbed
    let testbed_uuid = TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(&format!("test-testbed-{testbed_uuid}")),
            schema::testbed::slug.eq(&format!("test-testbed-{testbed_uuid}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert testbed");
    let testbed_id: i32 = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&testbed_uuid))
        .select(schema::testbed::id)
        .first(&mut conn)
        .expect("get testbed id");

    // Version
    let version_uuid = VersionUuid::new();
    if let Some(hash) = &opts.version_hash {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(&version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(opts.version_number),
                schema::version::hash.eq(hash),
            ))
            .execute(&mut conn)
            .expect("insert version with hash");
    } else {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(&version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(opts.version_number),
            ))
            .execute(&mut conn)
            .expect("insert version");
    }
    let version_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get version id");

    // Branch (without head_id first)
    let branch_uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(&branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq(&format!("main-{branch_uuid}")),
            schema::branch::slug.eq(&format!("main-{branch_uuid}")),
            schema::branch::created.eq(&now),
            schema::branch::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert branch");
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(&branch_uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("get branch id");

    // Head
    let head_uuid = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(&head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert head");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(&head_uuid))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("get head id");

    // UPDATE branch.head_id — critical for the perf query default head filter
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(&mut conn)
        .expect("update branch head_id");

    // head_version — critical for the perf query join chain
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version");

    // Report
    let report_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&report_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(head_id),
            schema::report::version_id.eq(version_id),
            schema::report::testbed_id.eq(testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&opts.start_time),
            schema::report::end_time.eq(&opts.end_time),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert report");
    let report_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&report_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report id");

    // Benchmark
    let benchmark_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(&format!("test-benchmark-{benchmark_uuid}")),
            schema::benchmark::slug.eq(&format!("test-benchmark-{benchmark_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("get benchmark id");
    let parameter_id = create_empty_parameter(&mut conn, benchmark_id);

    // Measure
    let measure_uuid = MeasureUuid::new();
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(&format!("test-measure-{measure_uuid}")),
            schema::measure::slug.eq(&format!("test-measure-{measure_uuid}")),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert measure");
    let measure_id: i32 = schema::measure::table
        .filter(schema::measure::uuid.eq(&measure_uuid))
        .select(schema::measure::id)
        .first(&mut conn)
        .expect("get measure id");

    // Report benchmark
    let report_benchmark_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&report_benchmark_uuid),
            schema::report_benchmark::report_id.eq(report_id),
            schema::report_benchmark::iteration.eq(opts.iteration),
            schema::report_benchmark::benchmark_id.eq(benchmark_id),
            schema::report_benchmark::parameter_id.eq(parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert report_benchmark");
    let report_benchmark_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get report_benchmark id");

    // Metric
    let metric_uuid = MetricUuid::new();
    create_metric(
        &mut conn,
        &metric_uuid,
        report_benchmark_id,
        measure_id,
        opts.metric_value,
        opts.lower_value,
        opts.upper_value,
    );

    // Keep metric_count_by_report in sync (1 metric inserted)
    let report_id_typed = ReportId::try_from_raw(report_id).expect("valid report ID");
    upsert_metric_count(&mut conn, report_id_typed, 1).expect("upsert metric_count_by_report");

    PerfTestData {
        branch_uuid,
        head_uuid,
        testbed_uuid,
        benchmark_uuid,
        measure_uuid,
        report_uuid,
        metric_uuid,
        branch_id,
        head_id,
        testbed_id,
        benchmark_id,
        parameter_id,
        measure_id,
        report_id,
        report_benchmark_id,
    }
}

// =============================================================================
// Helper: build_perf_url
// =============================================================================

fn build_perf_url(
    project_slug: &str,
    branches: &[BranchUuid],
    testbeds: &[TestbedUuid],
    benchmarks: &[BenchmarkUuid],
    measures: &[MeasureUuid],
    extra: &str,
) -> String {
    let branches_str: Vec<String> = branches.iter().map(ToString::to_string).collect();
    let testbeds_str: Vec<String> = testbeds.iter().map(ToString::to_string).collect();
    let benchmarks_str: Vec<String> = benchmarks.iter().map(ToString::to_string).collect();
    let measures_str: Vec<String> = measures.iter().map(ToString::to_string).collect();
    format!(
        "/v0/projects/{project_slug}/perf?branches={}&testbeds={}&benchmarks={}&measures={}{}",
        branches_str.join(","),
        testbeds_str.join(","),
        benchmarks_str.join(","),
        measures_str.join(","),
        extra,
    )
}

// =============================================================================
// Helper: set_project_private
// =============================================================================

fn set_project_private(server: &TestServer, project_uuid: bencher_json::ProjectUuid) {
    use bencher_json::project::Visibility;
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(project_uuid)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut conn)
        .expect("update project visibility");
}

// =============================================================================
// Helper: create_spec
// =============================================================================

fn create_spec(server: &TestServer) -> (SpecUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let spec_uuid = SpecUuid::new();
    let spec_name = format!("perf-test-spec-{spec_uuid}");
    let spec_slug = format!("perf-test-spec-{spec_uuid}");
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::name.eq(&spec_name),
            schema::spec::slug.eq(&spec_slug),
            schema::spec::os.eq("linux"),
            schema::spec::architecture.eq("x86_64"),
            schema::spec::cpu.eq(4),
            schema::spec::memory.eq(0x0002_0000_0000i64),
            schema::spec::disk.eq(0x0005_0000_0000i64),
            schema::spec::network.eq(true),
            schema::spec::created.eq(&now),
            schema::spec::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert spec");
    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec_uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("get spec id");
    (spec_uuid, spec_id)
}

// =============================================================================
// Helper: create_job
// =============================================================================

fn create_job(server: &TestServer, report_id: i32, spec_id: i32, project_id: i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let organization_id: i32 = schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(&mut conn)
        .expect("get organization id");
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": bencher_json::ProjectUuid::new(),
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": 300
    });
    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq("127.0.0.1"),
            schema::job::status.eq(JobStatus::Completed),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(300),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("set report spec_id");
}

// =============================================================================
// Helper: create_threshold_and_boundary
// =============================================================================

/// Create a threshold + model + boundary for a given metric, returning IDs for alert creation.
fn create_threshold_and_boundary(
    server: &TestServer,
    data: &PerfTestData,
    project_id: i32,
) -> (i32, i32) {
    use bencher_json::ThresholdUuid;
    let mut conn = server.db_conn();
    let now = base_timestamp();

    // Threshold
    let threshold_uuid = ThresholdUuid::new();
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(&threshold_uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(data.branch_id),
            schema::threshold::testbed_id.eq(data.testbed_id),
            schema::threshold::measure_id.eq(data.measure_id),
            schema::threshold::created.eq(&now),
            schema::threshold::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert threshold");
    let threshold_id: i32 = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&threshold_uuid))
        .select(schema::threshold::id)
        .first(&mut conn)
        .expect("get threshold id");

    // Model
    let model_uuid = bencher_json::ModelUuid::new();
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(&model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(0), // static test
            schema::model::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert model");
    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(&model_uuid))
        .select(schema::model::id)
        .first(&mut conn)
        .expect("get model id");

    // Update threshold to reference model
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(&mut conn)
        .expect("update threshold model_id");

    // Boundary
    let boundary_uuid = BoundaryUuid::new();
    // Get metric_id from the metric table
    let metric_id: i32 = schema::metric::table
        .filter(schema::metric::uuid.eq(&data.metric_uuid))
        .select(schema::metric::id)
        .first(&mut conn)
        .expect("get metric id");

    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(&boundary_uuid),
            schema::boundary::metric_id.eq(metric_id),
            schema::boundary::threshold_id.eq(threshold_id),
            schema::boundary::model_id.eq(model_id),
            schema::boundary::baseline.eq(Some(100.0)),
            schema::boundary::lower_limit.eq(Some(50.0)),
            schema::boundary::upper_limit.eq(Some(150.0)),
        ))
        .execute(&mut conn)
        .expect("insert boundary");
    let boundary_id: i32 = schema::boundary::table
        .filter(schema::boundary::uuid.eq(&boundary_uuid))
        .select(schema::boundary::id)
        .first(&mut conn)
        .expect("get boundary id");

    (threshold_id, boundary_id)
}

/// Create an alert for a boundary.
fn create_alert(server: &TestServer, boundary_id: i32) -> AlertUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let alert_uuid = AlertUuid::new();
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(&alert_uuid),
            schema::alert::boundary_id.eq(boundary_id),
            schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
            schema::alert::status.eq(AlertStatus::Active),
            schema::alert::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert alert");
    alert_uuid
}

// =============================================================================
// Section 1: Basic happy path
// =============================================================================

#[tokio::test]
async fn perf_get_single_result() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfsingle@example.com").await;
    let org = server.create_org(&user, "Perf Single Org").await;
    let project = server
        .create_project(&user, &org, "Perf Single Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        42.0
    );
    assert_eq!(perf.results[0].metrics[0].report, data.report_uuid);
}

#[tokio::test]
async fn perf_get_response_structure() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfstructure@example.com")
        .await;
    let org = server.create_org(&user, "Perf Structure Org").await;
    let project = server
        .create_project(&user, &org, "Perf Structure Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Verify top-level structure
    assert_eq!(perf.project.uuid, project.uuid);
    // The query named no window, so the response carries the default one.
    assert_eq!(
        perf.start_time.expect("the window it plotted").timestamp(),
        base_timestamp().timestamp() - REPORT_HISTORY_SECS
    );
    assert!(perf.end_time.is_none());
    // Verify result dimensions
    let result = &perf.results[0];
    assert_eq!(result.branch.uuid, data.branch_uuid);
    assert_eq!(result.testbed.uuid, data.testbed_uuid);
    assert_eq!(result.benchmark.uuid, data.benchmark_uuid);
    assert_eq!(result.measure.uuid, data.measure_uuid);
    // Verify metric fields
    let metric = &result.metrics[0];
    assert_eq!(metric.iteration.0, 0);
    assert!(metric.threshold.is_none());
    assert!(metric.boundary.is_none());
    assert!(metric.alert.is_none());
}

#[tokio::test]
async fn perf_get_empty_results() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfempty@example.com").await;
    let org = server.create_org(&user, "Perf Empty Org").await;
    let project = server
        .create_project(&user, &org, "Perf Empty Project")
        .await;

    // Create data but query with a different (nonexistent) branch UUID
    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    let fake_branch = BranchUuid::new();

    let url = build_perf_url(
        project.slug.as_ref(),
        &[fake_branch],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_get_multiple_metrics_same_permutation() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmultimetric@example.com")
        .await;
    let org = server.create_org(&user, "Perf MultiMetric Org").await;
    let project = server
        .create_project(&user, &org, "Perf MultiMetric Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Add a second report with version_number=2 on the same branch/testbed/benchmark/measure
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let ts2 = bencher_json::DateTime::try_from(now.timestamp() + 1).expect("valid ts");

    let version2_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version2_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(2),
        ))
        .execute(&mut conn)
        .expect("insert version2");
    let version2_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version2_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get version2 id");

    // Link version2 to the same head
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(data.head_id),
            schema::head_version::version_id.eq(version2_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version2");

    let report2_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&report2_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(data.head_id),
            schema::report::version_id.eq(version2_id),
            schema::report::testbed_id.eq(data.testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&ts2),
            schema::report::end_time.eq(&ts2),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert report2");
    let report2_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&report2_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report2 id");

    let rb2_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&rb2_uuid),
            schema::report_benchmark::report_id.eq(report2_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(data.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert rb2");
    let rb2_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&rb2_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get rb2 id");

    let metric2_uuid = MetricUuid::new();
    create_metric(
        &mut conn,
        &metric2_uuid,
        rb2_id,
        data.measure_id,
        99.0,
        None,
        None,
    );

    // Query perf
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Ordered by version number (oldest first)
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        42.0
    );
    assert_eq!(
        perf.results[0].metrics[1]
            .metric
            .expect("the metric triple")
            .value,
        99.0
    );
}

// =============================================================================
// Section 2: Query filtering
// =============================================================================

#[tokio::test]
async fn perf_filter_by_start_time() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfstarttime@example.com")
        .await;
    let org = server.create_org(&user, "Perf StartTime Org").await;
    let project = server
        .create_project(&user, &org, "Perf StartTime Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let ts = base_timestamp();
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            start_time: ts,
            end_time: ts,
            ..Default::default()
        },
    );

    // start_time filter is in milliseconds, set after base_timestamp
    let after_ms = (ts.timestamp() + 1) * 1000;
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&start_time={after_ms}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // The report's start_time is base_timestamp, which is before our filter
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_filter_by_end_time() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfendtime@example.com").await;
    let org = server.create_org(&user, "Perf EndTime Org").await;
    let project = server
        .create_project(&user, &org, "Perf EndTime Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let ts = base_timestamp();
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            start_time: ts,
            end_time: ts,
            ..Default::default()
        },
    );

    // end_time filter before the report's end_time
    let before_ms = (ts.timestamp() - 1) * 1000;
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&end_time={before_ms}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_filter_includes_matching_time() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perftimematch@example.com")
        .await;
    let org = server.create_org(&user, "Perf TimeMatch Org").await;
    let project = server
        .create_project(&user, &org, "Perf TimeMatch Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let ts = base_timestamp();
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            start_time: ts,
            end_time: ts,
            ..Default::default()
        },
    );

    // Use exact timestamp in ms — should include the result (GE/LE)
    let exact_ms = ts.timestamp() * 1000;
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&start_time={exact_ms}&end_time={exact_ms}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
}

#[tokio::test]
async fn perf_multi_branch_query() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmultibranch@example.com")
        .await;
    let org = server.create_org(&user, "Perf MultiBranch Org").await;
    let project = server
        .create_project(&user, &org, "Perf MultiBranch Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data1 = create_perf_data(&server, project_id);
    let data2 = create_perf_data(&server, project_id);

    // Query with both branches but shared testbed/benchmark/measure from data1 → only data1 matches
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data1.branch_uuid, data2.branch_uuid],
        &[data1.testbed_uuid],
        &[data1.benchmark_uuid],
        &[data1.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // data1's branch matches, data2's branch won't match data1's testbed/benchmark/measure
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].branch.uuid, data1.branch_uuid);
}

#[tokio::test]
async fn perf_multi_testbed_query() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmultitestbed@example.com")
        .await;
    let org = server.create_org(&user, "Perf MultiTestbed Org").await;
    let project = server
        .create_project(&user, &org, "Perf MultiTestbed Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data1 = create_perf_data(&server, project_id);
    let data2 = create_perf_data(&server, project_id);

    // Query with both testbeds but data1's branch/benchmark/measure
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data1.branch_uuid],
        &[data1.testbed_uuid, data2.testbed_uuid],
        &[data1.benchmark_uuid],
        &[data1.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Only the permutation matching data1's testbed has results
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].testbed.uuid, data1.testbed_uuid);
}

#[tokio::test]
async fn perf_multi_measure_query() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmultimeasure@example.com")
        .await;
    let org = server.create_org(&user, "Perf MultiMeasure Org").await;
    let project = server
        .create_project(&user, &org, "Perf MultiMeasure Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Add a second measure and metric on the same report_benchmark
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let measure2_uuid = MeasureUuid::new();
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure2_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq("test-measure-2"),
            schema::measure::slug.eq(&format!("test-measure-2-{measure2_uuid}")),
            schema::measure::units.eq("bytes"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert measure2");
    let measure2_id: i32 = schema::measure::table
        .filter(schema::measure::uuid.eq(&measure2_uuid))
        .select(schema::measure::id)
        .first(&mut conn)
        .expect("get measure2 id");

    let metric2_uuid = MetricUuid::new();
    create_metric(
        &mut conn,
        &metric2_uuid,
        data.report_benchmark_id,
        measure2_id,
        1024.0,
        None,
        None,
    );

    // Query with both measures
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid, measure2_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 2);
}

#[tokio::test]
async fn perf_multi_benchmark_query() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmultibenchmark@example.com")
        .await;
    let org = server.create_org(&user, "Perf MultiBenchmark Org").await;
    let project = server
        .create_project(&user, &org, "Perf MultiBenchmark Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Add a second benchmark and report_benchmark+metric on the same report
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let benchmark2_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark2_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("test-benchmark-2"),
            schema::benchmark::slug.eq(&format!("test-benchmark-2-{benchmark2_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert benchmark2");
    let benchmark2_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark2_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("get benchmark2 id");
    let parameter2_id = create_empty_parameter(&mut conn, benchmark2_id);

    let report_benchmark2_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&report_benchmark2_uuid),
            schema::report_benchmark::report_id.eq(data.report_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(benchmark2_id),
            schema::report_benchmark::parameter_id.eq(parameter2_id),
        ))
        .execute(&mut conn)
        .expect("insert report_benchmark2");
    let report_benchmark2_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark2_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get report_benchmark2 id");

    let metric2_uuid = MetricUuid::new();
    create_metric(
        &mut conn,
        &metric2_uuid,
        report_benchmark2_id,
        data.measure_id,
        99.0,
        None,
        None,
    );

    // Query with both benchmarks
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid, benchmark2_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 2);
}

// =============================================================================
// Section 3: Input validation
// =============================================================================

#[tokio::test]
async fn perf_missing_branches_param() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnobranch@example.com").await;
    let org = server.create_org(&user, "Perf NoBranch Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoBranch Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = TestbedUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?testbeds={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_testbeds_param() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfnotestbed@example.com")
        .await;
    let org = server.create_org(&user, "Perf NoTestbed Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoTestbed Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?branches={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_benchmarks_param() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnobench@example.com").await;
    let org = server.create_org(&user, "Perf NoBench Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoBench Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?branches={uuid}&testbeds={uuid}&measures={uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_measures_param() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfnomeasure@example.com")
        .await;
    let org = server.create_org(&user, "Perf NoMeasure Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoMeasure Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?branches={uuid}&testbeds={uuid}&benchmarks={uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_invalid_branch_uuid() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfinvbranch@example.com")
        .await;
    let org = server.create_org(&user, "Perf InvBranch Org").await;
    let project = server
        .create_project(&user, &org, "Perf InvBranch Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = TestbedUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?branches=not-a-uuid&testbeds={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header(bencher_json::AUTHORIZATION, bencher_json::bearer_header(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_nonexistent_project() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnoproj@example.com").await;

    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/nonexistent-project/perf?branches={uuid}&testbeds={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header(bencher_json::AUTHORIZATION, bencher_json::bearer_header(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn perf_no_query_params() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnoparams@example.com").await;
    let org = server.create_org(&user, "Perf NoParams Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoParams Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/perf")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_empty_branches_value() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfemptybranch@example.com")
        .await;
    let org = server.create_org(&user, "Perf EmptyBranch Org").await;
    let project = server
        .create_project(&user, &org, "Perf EmptyBranch Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/perf?branches=&testbeds={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header(bencher_json::AUTHORIZATION, bencher_json::bearer_header(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// Section 4: Auth
// =============================================================================

#[tokio::test]
async fn perf_public_project_no_auth() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfpublicnoauth@example.com")
        .await;
    let org = server.create_org(&user, "Perf PublicNoAuth Org").await;
    let project = server
        .create_project(&user, &org, "Perf PublicNoAuth Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    // No auth header — public project
    let resp = server
        .client
        .get(server.api_url(&url))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

#[tokio::test]
async fn perf_private_project_no_auth() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfprivnoauth@example.com")
        .await;
    let org = server.create_org(&user, "Perf PrivNoAuth Org").await;
    let project = server
        .create_project(&user, &org, "Perf PrivNoAuth Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    set_project_private(&server, project.uuid);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .send()
        .await
        .expect("Request failed");

    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN for private project, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn perf_private_project_with_auth() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfprivauth@example.com").await;
    let org = server.create_org(&user, "Perf PrivAuth Org").await;
    let project = server
        .create_project(&user, &org, "Perf PrivAuth Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    set_project_private(&server, project.uuid);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

#[tokio::test]
async fn perf_private_project_wrong_user() {
    let server = perf_server().await;
    let owner = server.signup("Owner", "perfprivowner@example.com").await;
    let other = server.signup("Other", "perfprivother@example.com").await;
    let org = server.create_org(&owner, "Perf PrivOther Org").await;
    let project = server
        .create_project(&owner, &org, "Perf PrivOther Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    set_project_private(&server, project.uuid);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&other.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN for non-member, got {}",
        resp.status()
    );
}

// =============================================================================
// Section 5: The line limit
// =============================================================================

#[tokio::test]
async fn perf_line_limit_exact_256() {
    let server = perf_server().await;
    // `4` branches by `64` measures is exactly the budget.
    let (user, project_slug, grid) = grid_project(&server, "limit256", 4, 1, 1, 64).await;

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let expected = grid.expected_lines();
    assert_eq!(expected.len(), 256);
    assert_eq!(perf.results.len(), 256);
    assert_eq!(response_lines(&perf), expected);
}

#[tokio::test]
async fn perf_line_limit_truncated_past_256() {
    let server = perf_server().await;
    // `5` branches by `64` measures is `320`.
    let (user, project_slug, grid) = grid_project(&server, "limitpast", 5, 1, 1, 64).await;

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 320);
    expected.truncate(256);
    assert_eq!(perf.results.len(), 256);
    assert_eq!(response_lines(&perf), expected);
}

// The budget is a running count of lines, not a product of dimension indexes.
#[tokio::test]
async fn perf_line_limit_holds_on_an_asymmetric_grid() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "asymmetric", 2, 2, 6, 21).await;

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 504);
    expected.truncate(256);
    assert_eq!(perf.results.len(), 256);
    assert_eq!(response_lines(&perf), expected);
}

#[tokio::test]
async fn perf_line_limit_holds_within_one_benchmark() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = grid_project(&server, "variants", 1, 1, 1, 1).await;
    // The empty parameter set, plus `299` more.
    for size in 0..299 {
        grid.add_variant(&server, 0, &format!(r#"{{"size_mb": {size}}}"#));
    }

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 300);
    expected.truncate(256);
    assert_eq!(perf.results.len(), 256);
    assert_eq!(response_lines(&perf), expected);
}

#[tokio::test]
async fn perf_line_limit_clips_a_fan_out() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = grid_project(&server, "clipped", 1, 1, 2, 1).await;
    for size in 0..199 {
        grid.add_variant(&server, 0, &format!(r#"{{"size_mb": {size}}}"#));
    }
    for size in 0..99 {
        grid.add_variant(&server, 1, &format!(r#"{{"size_mb": {size}}}"#));
    }

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 300);
    expected.truncate(256);
    assert_eq!(perf.results.len(), 256);
    assert_eq!(response_lines(&perf), expected);

    // The first benchmark spent `200`, so `56` of the second's `100` variants are
    // plotted, and they are the first `56`.
    let clipped = perf
        .results
        .iter()
        .filter(|result| result.benchmark.uuid == grid.benchmarks[1].0)
        .count();
    assert_eq!(clipped, 56);
}

#[tokio::test]
async fn perf_line_limit_refunds_empty_permutations() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = empty_grid_project(&server, "refund", 3, 1, 1, 1).await;
    // Reported on the last branch only, so the two permutations before it hold
    // nothing.
    for size in 0..99 {
        grid.add_variant(&server, 0, &format!(r#"{{"size_mb": {size}}}"#));
    }
    grid.report(&server, 2, 0);

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let expected = grid.expected_lines();
    assert_eq!(expected.len(), 100, "one line per variant, on one branch");
    assert_eq!(perf.results.len(), 100);
    assert_eq!(response_lines(&perf), expected);
}

#[tokio::test]
async fn perf_line_limit_bounds_the_permutations_queried() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) =
        empty_grid_project(&server, "workbound", 5, 1, 1, 64).await;
    // `192` permutations that hold nothing, leaving `64` of the `256` for the
    // branches that do.
    grid.report(&server, 3, 0);
    grid.report(&server, 4, 0);

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 128, "the two branches that reported");
    expected.truncate(64);
    assert_eq!(perf.results.len(), 64);
    assert_eq!(response_lines(&perf), expected);
}

#[tokio::test]
async fn perf_truncates_the_benchmarks_list() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "benchmarklist", 1, 1, 8, 1).await;

    // `60` benchmarks that do not exist, then the first `4` that do, then the
    // last `4` past the `64`th entry.
    let mut query = grid.query();
    let mut benchmarks = std::iter::repeat_with(BenchmarkUuid::new)
        .take(60)
        .collect::<Vec<_>>();
    benchmarks.extend(grid.benchmarks.iter().map(|(uuid, _)| *uuid));
    query.benchmarks = benchmarks;
    assert_eq!(query.benchmarks.len(), 68);

    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    let plotted = perf
        .results
        .iter()
        .map(|result| result.benchmark.uuid)
        .collect::<Vec<_>>();
    assert_eq!(
        plotted,
        grid.benchmarks
            .iter()
            .take(4)
            .map(|(uuid, _)| *uuid)
            .collect::<Vec<_>>(),
        "the four benchmarks inside the first sixty four entries"
    );
}

#[tokio::test]
async fn perf_truncates_the_measures_list() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "measurelist", 1, 1, 1, 70).await;

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    let mut expected = grid.expected_lines();
    assert_eq!(expected.len(), 70);
    expected.truncate(64);
    assert_eq!(response_lines(&perf), expected);
}

// =============================================================================
// Section 6: Threshold / boundary / alert
// =============================================================================

#[tokio::test]
async fn perf_without_threshold() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfnothreshold@example.com")
        .await;
    let org = server.create_org(&user, "Perf NoThreshold Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoThreshold Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let metric = &perf.results[0].metrics[0];
    assert!(metric.threshold.is_none());
    assert!(metric.boundary.is_none());
    assert!(metric.alert.is_none());
}

#[tokio::test]
async fn perf_with_boundary() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfboundary@example.com").await;
    let org = server.create_org(&user, "Perf Boundary Org").await;
    let project = server
        .create_project(&user, &org, "Perf Boundary Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    let (_threshold_id, _boundary_id) = create_threshold_and_boundary(&server, &data, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let metric = &perf.results[0].metrics[0];
    assert!(metric.threshold.is_some());
    let boundary = metric
        .boundary
        .as_ref()
        .expect("boundary should be present");
    assert_eq!(boundary.baseline, Some(100.0.into()));
    assert_eq!(boundary.lower_limit, Some(50.0.into()));
    assert_eq!(boundary.upper_limit, Some(150.0.into()));
    assert!(metric.alert.is_none());
}

#[tokio::test]
async fn perf_with_alert() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfalert@example.com").await;
    let org = server.create_org(&user, "Perf Alert Org").await;
    let project = server
        .create_project(&user, &org, "Perf Alert Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    let (_threshold_id, boundary_id) = create_threshold_and_boundary(&server, &data, project_id);
    let alert_uuid = create_alert(&server, boundary_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let metric = &perf.results[0].metrics[0];
    assert!(metric.threshold.is_some());
    assert!(metric.boundary.is_some());
    let alert = metric.alert.as_ref().expect("alert should be present");
    assert_eq!(alert.uuid, alert_uuid);
    assert_eq!(alert.limit, BoundaryLimit::Upper);
}

// =============================================================================
// Section 7: Ordering
// =============================================================================

#[tokio::test]
async fn perf_ordered_by_version_number() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perforderversion@example.com")
        .await;
    let org = server.create_org(&user, "Perf OrderVersion Org").await;
    let project = server
        .create_project(&user, &org, "Perf OrderVersion Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let now = base_timestamp();

    // Create data with version 2 first, then version 1
    let data_v2 = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            version_number: 2,
            metric_value: 200.0,
            start_time: now,
            end_time: now,
            ..Default::default()
        },
    );

    // Create a second set with version 1, sharing the same branch
    // We need to add version 1 to the same head
    let mut conn = server.db_conn();
    let v1_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&v1_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut conn)
        .expect("insert v1");
    let v1_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&v1_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get v1 id");

    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(data_v2.head_id),
            schema::head_version::version_id.eq(v1_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version v1");

    let report_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&report_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(data_v2.head_id),
            schema::report::version_id.eq(v1_id),
            schema::report::testbed_id.eq(data_v2.testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&now),
            schema::report::end_time.eq(&now),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert report v1");
    let report_v1_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&report_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report v1 id");

    let rb_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&rb_uuid),
            schema::report_benchmark::report_id.eq(report_v1_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(data_v2.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data_v2.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert rb v1");
    let rb_v1_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&rb_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get rb v1 id");

    let m_uuid = MetricUuid::new();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&m_uuid),
            schema::metric::report_benchmark_id.eq(rb_v1_id),
            schema::metric::measure_id.eq(data_v2.measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(100.0),
        ))
        .execute(&mut conn)
        .expect("insert metric v1");

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data_v2.branch_uuid],
        &[data_v2.testbed_uuid],
        &[data_v2.benchmark_uuid],
        &[data_v2.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // v1 should come first (oldest version number)
    assert_eq!(perf.results[0].metrics[0].version.number.0, 1);
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        100.0
    );
    assert_eq!(perf.results[0].metrics[1].version.number.0, 2);
    assert_eq!(
        perf.results[0].metrics[1]
            .metric
            .expect("the metric triple")
            .value,
        200.0
    );
}

#[tokio::test]
async fn perf_ordered_by_start_time_within_version() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfordertime@example.com")
        .await;
    let org = server.create_org(&user, "Perf OrderTime Org").await;
    let project = server
        .create_project(&user, &org, "Perf OrderTime Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let ts1 = base_timestamp();
    let ts2 = bencher_json::DateTime::try_from(ts1.timestamp() + 10).expect("valid ts");
    let now = base_timestamp();

    // Create first data with ts2 (later time)
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            version_number: 1,
            metric_value: 200.0,
            start_time: ts2,
            end_time: ts2,
            ..Default::default()
        },
    );

    // Add a second report with ts1 (earlier time), same version number
    let mut conn = server.db_conn();
    let v_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&v_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut conn)
        .expect("insert version");
    let v_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&v_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get version id");

    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(data.head_id),
            schema::head_version::version_id.eq(v_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version");

    let r_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&r_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(data.head_id),
            schema::report::version_id.eq(v_id),
            schema::report::testbed_id.eq(data.testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&ts1),
            schema::report::end_time.eq(&ts1),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert report");
    let r_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&r_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report id");

    let rb_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&rb_uuid),
            schema::report_benchmark::report_id.eq(r_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(data.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert rb");
    let rb_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&rb_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get rb id");

    let m_uuid = MetricUuid::new();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&m_uuid),
            schema::metric::report_benchmark_id.eq(rb_id),
            schema::metric::measure_id.eq(data.measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(100.0),
        ))
        .execute(&mut conn)
        .expect("insert metric");

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Earlier start_time should come first (within same version number)
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        100.0
    );
    assert_eq!(
        perf.results[0].metrics[1]
            .metric
            .expect("the metric triple")
            .value,
        200.0
    );
}

#[tokio::test]
async fn perf_version_hash_returned() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfversionhash@example.com")
        .await;
    let org = server.create_org(&user, "Perf VersionHash Org").await;
    let project = server
        .create_project(&user, &org, "Perf VersionHash Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            version_hash: Some("abc1234567890abc1234567890abc1234567890a".to_owned()),
            ..Default::default()
        },
    );

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let version = &perf.results[0].metrics[0].version;
    assert!(version.hash.is_some());
    assert!(
        version
            .hash
            .as_ref()
            .expect("hash should be present")
            .as_ref()
            .starts_with("abc1234")
    );
}

// =============================================================================
// Section 8: Branch head
// =============================================================================

#[tokio::test]
async fn perf_default_head() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfdefaulthead@example.com")
        .await;
    let org = server.create_org(&user, "Perf DefaultHead Org").await;
    let project = server
        .create_project(&user, &org, "Perf DefaultHead Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // No explicit head in query — should use branch.head_id
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

#[tokio::test]
async fn perf_explicit_head() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfexplicithead@example.com")
        .await;
    let org = server.create_org(&user, "Perf ExplicitHead Org").await;
    let project = server
        .create_project(&user, &org, "Perf ExplicitHead Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Explicitly provide the head UUID
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&heads={}", data.head_uuid),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
}

#[tokio::test]
async fn perf_no_head_id_set_on_branch() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnoheadid@example.com").await;
    let org = server.create_org(&user, "Perf NoHeadId Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoHeadId Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Clear branch.head_id to simulate missing head
    let mut conn = server.db_conn();
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(data.branch_id)))
        .set(schema::branch::head_id.eq(None::<i32>))
        .execute(&mut conn)
        .expect("clear branch head_id");

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Without branch.head_id, the default filter (branch.head_id == head.id) won't match
    assert!(perf.results.is_empty());
}

// =============================================================================
// Section 9: Cross-project isolation
// =============================================================================

#[tokio::test]
async fn perf_wrong_project_branch() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfwrongprojbranch@example.com")
        .await;
    let org = server.create_org(&user, "Perf WrongProjBranch Org").await;
    let project_a = server.create_project(&user, &org, "Perf WrongProj A").await;
    let project_b = server.create_project(&user, &org, "Perf WrongProj B").await;

    let project_a_id = get_project_id(&server, project_a.slug.as_ref());
    let data_a = create_perf_data(&server, project_a_id);
    let project_b_id = get_project_id(&server, project_b.slug.as_ref());
    let data_b = create_perf_data(&server, project_b_id);

    // Use project_b's branch in project_a's query
    let url = build_perf_url(
        project_a.slug.as_ref(),
        &[data_b.branch_uuid],
        &[data_a.testbed_uuid],
        &[data_a.benchmark_uuid],
        &[data_a.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Branch belongs to project_b, so project_id filter prevents match
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_wrong_project_testbed() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfwrongprojtestbed@example.com")
        .await;
    let org = server.create_org(&user, "Perf WrongProjTestbed Org").await;
    let project_a = server
        .create_project(&user, &org, "Perf WrongProjTb A")
        .await;
    let project_b = server
        .create_project(&user, &org, "Perf WrongProjTb B")
        .await;

    let project_a_id = get_project_id(&server, project_a.slug.as_ref());
    let data_a = create_perf_data(&server, project_a_id);
    let project_b_id = get_project_id(&server, project_b.slug.as_ref());
    let data_b = create_perf_data(&server, project_b_id);

    // Use project_b's testbed in project_a's query
    let url = build_perf_url(
        project_a.slug.as_ref(),
        &[data_a.branch_uuid],
        &[data_b.testbed_uuid],
        &[data_a.benchmark_uuid],
        &[data_a.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_wrong_project_benchmark() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfwrongprojbench@example.com")
        .await;
    let org = server.create_org(&user, "Perf WrongProjBench Org").await;
    let project_a = server
        .create_project(&user, &org, "Perf WrongProjBn A")
        .await;
    let project_b = server
        .create_project(&user, &org, "Perf WrongProjBn B")
        .await;

    let project_a_id = get_project_id(&server, project_a.slug.as_ref());
    let data_a = create_perf_data(&server, project_a_id);
    let project_b_id = get_project_id(&server, project_b.slug.as_ref());
    let data_b = create_perf_data(&server, project_b_id);

    let url = build_perf_url(
        project_a.slug.as_ref(),
        &[data_a.branch_uuid],
        &[data_a.testbed_uuid],
        &[data_b.benchmark_uuid],
        &[data_a.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_wrong_project_measure() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfwrongprojmeasure@example.com")
        .await;
    let org = server.create_org(&user, "Perf WrongProjMeasure Org").await;
    let project_a = server
        .create_project(&user, &org, "Perf WrongProjMs A")
        .await;
    let project_b = server
        .create_project(&user, &org, "Perf WrongProjMs B")
        .await;

    let project_a_id = get_project_id(&server, project_a.slug.as_ref());
    let data_a = create_perf_data(&server, project_a_id);
    let project_b_id = get_project_id(&server, project_b.slug.as_ref());
    let data_b = create_perf_data(&server, project_b_id);

    let url = build_perf_url(
        project_a.slug.as_ref(),
        &[data_a.branch_uuid],
        &[data_a.testbed_uuid],
        &[data_a.benchmark_uuid],
        &[data_b.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

// =============================================================================
// Section 10: Edge cases
// =============================================================================

#[tokio::test]
async fn perf_lower_upper_values() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perflowerupper@example.com")
        .await;
    let org = server.create_org(&user, "Perf LowerUpper Org").await;
    let project = server
        .create_project(&user, &org, "Perf LowerUpper Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            metric_value: 100.0,
            lower_value: Some(90.0),
            upper_value: Some(110.0),
            ..Default::default()
        },
    );

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let metric = perf.results[0].metrics[0]
        .metric
        .expect("the metric triple");
    assert_eq!(metric.value, 100.0);
    assert_eq!(metric.lower_value, Some(90.0.into()));
    assert_eq!(metric.upper_value, Some(110.0.into()));
}

#[tokio::test]
async fn perf_multiple_iterations() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfiterations@example.com")
        .await;
    let org = server.create_org(&user, "Perf Iterations Org").await;
    let project = server
        .create_project(&user, &org, "Perf Iterations Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            iteration: 0,
            metric_value: 10.0,
            ..Default::default()
        },
    );

    // Add iteration 1 on the same report
    let mut conn = server.db_conn();
    let rb2_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&rb2_uuid),
            schema::report_benchmark::report_id.eq(data.report_id),
            schema::report_benchmark::iteration.eq(1),
            schema::report_benchmark::benchmark_id.eq(data.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert rb iter1");
    let rb2_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&rb2_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get rb iter1 id");

    let m2_uuid = MetricUuid::new();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&m2_uuid),
            schema::metric::report_benchmark_id.eq(rb2_id),
            schema::metric::measure_id.eq(data.measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(20.0),
        ))
        .execute(&mut conn)
        .expect("insert metric iter1");

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Ordered by iteration
    assert_eq!(perf.results[0].metrics[0].iteration.0, 0);
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        10.0
    );
    assert_eq!(perf.results[0].metrics[1].iteration.0, 1);
    assert_eq!(
        perf.results[0].metrics[1]
            .metric
            .expect("the metric triple")
            .value,
        20.0
    );
}

#[tokio::test]
async fn perf_time_echo() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perftimeecho@example.com").await;
    let org = server.create_org(&user, "Perf TimeEcho Org").await;
    let project = server
        .create_project(&user, &org, "Perf TimeEcho Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let ts = base_timestamp();
    let ts_end = bencher_json::DateTime::try_from(ts.timestamp() + 5).expect("valid ts");
    let data = create_perf_data_with_options(
        &server,
        project_id,
        &PerfDataOptions {
            start_time: ts,
            end_time: ts_end,
            ..Default::default()
        },
    );

    let start_ms = ts.timestamp() * 1000;
    let end_ms = ts_end.timestamp() * 1000;
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&start_time={start_ms}&end_time={end_ms}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // The times passed as query params should be echoed back
    assert!(perf.start_time.is_some());
    assert!(perf.end_time.is_some());
    assert_eq!(
        perf.start_time
            .expect("start_time should be echoed back")
            .timestamp(),
        ts.timestamp()
    );
    assert_eq!(
        perf.end_time
            .expect("end_time should be echoed back")
            .timestamp(),
        ts_end.timestamp()
    );
}

// =============================================================================
// Section 11: Testbed / spec
// =============================================================================

#[tokio::test]
async fn perf_spec_from_query_param() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfspecquery@example.com")
        .await;
    let org = server.create_org(&user, "Perf SpecQuery Org").await;
    let project = server
        .create_project(&user, &org, "Perf SpecQuery Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    let (spec_uuid, spec_id) = create_spec(&server);
    create_job(&server, data.report_id, spec_id, project_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&specs={spec_uuid}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    let spec = perf.results[0]
        .testbed
        .spec
        .as_ref()
        .expect("spec should be present when queried");
    assert_eq!(spec.uuid, spec_uuid);
}

#[tokio::test]
async fn perf_no_spec_when_omitted() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnospec@example.com").await;
    let org = server.create_org(&user, "Perf NoSpec Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoSpec Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Even if testbed has a spec set in DB, the perf endpoint only uses query param
    let (_spec_uuid, spec_id) = create_spec(&server);
    let mut conn = server.db_conn();
    diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(data.testbed_id)))
        .set(schema::testbed::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("set testbed spec");

    // Query WITHOUT specs param
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    // spec should be None when specs query param is omitted
    assert!(perf.results[0].testbed.spec.is_none());
}

#[tokio::test]
async fn perf_spec_empty_entry() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfspecempty@example.com")
        .await;
    let org = server.create_org(&user, "Perf SpecEmpty Org").await;
    let project = server
        .create_project(&user, &org, "Perf SpecEmpty Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Empty entry in specs list (comma with nothing) → None for that testbed
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "&specs=",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert!(perf.results[0].testbed.spec.is_none());
}

#[tokio::test]
async fn perf_spec_filters_results() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfspecfilter@example.com")
        .await;
    let org = server.create_org(&user, "Perf SpecFilter Org").await;
    let project = server
        .create_project(&user, &org, "Perf SpecFilter Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());

    // Create first report (version_number=1)
    let data1 = create_perf_data(&server, project_id);

    // Create second report (version_number=2) sharing the same branch/testbed/benchmark/measure
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let ts2 = bencher_json::DateTime::try_from(now.timestamp() + 1).expect("valid ts");

    let version2_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version2_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(2),
        ))
        .execute(&mut conn)
        .expect("insert version2");
    let version2_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version2_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get version2 id");

    // Link version2 to the same head
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(data1.head_id),
            schema::head_version::version_id.eq(version2_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version2");

    let report2_uuid = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(&report2_uuid),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(data1.head_id),
            schema::report::version_id.eq(version2_id),
            schema::report::testbed_id.eq(data1.testbed_id),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(&ts2),
            schema::report::end_time.eq(&ts2),
            schema::report::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert report2");
    let report2_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&report2_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report2 id");

    let rb2_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&rb2_uuid),
            schema::report_benchmark::report_id.eq(report2_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(data1.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data1.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert rb2");
    let rb2_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&rb2_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get rb2 id");

    let metric2_uuid = MetricUuid::new();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&metric2_uuid),
            schema::metric::report_benchmark_id.eq(rb2_id),
            schema::metric::measure_id.eq(data1.measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(99.0),
        ))
        .execute(&mut conn)
        .expect("insert metric2");
    drop(conn);

    // Create two specs
    let (spec_a_uuid, spec_a_id) = create_spec(&server);
    let (spec_b_uuid, spec_b_id) = create_spec(&server);

    // Link report_1 → spec_a, report_2 → spec_b via jobs
    create_job(&server, data1.report_id, spec_a_id, project_id);
    create_job(&server, report2_id, spec_b_id, project_id);

    // Query with spec_a → only 1 metric (from report_1)
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data1.branch_uuid],
        &[data1.testbed_uuid],
        &[data1.benchmark_uuid],
        &[data1.measure_uuid],
        &format!("&specs={spec_a_uuid}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
    assert_eq!(perf.results[0].metrics[0].report, data1.report_uuid);

    // Query with spec_b → only 1 metric (from report_2)
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data1.branch_uuid],
        &[data1.testbed_uuid],
        &[data1.benchmark_uuid],
        &[data1.measure_uuid],
        &format!("&specs={spec_b_uuid}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
    assert_eq!(perf.results[0].metrics[0].report, report2_uuid);

    // Query without specs → both metrics returned
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data1.branch_uuid],
        &[data1.testbed_uuid],
        &[data1.benchmark_uuid],
        &[data1.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 2);
}

#[tokio::test]
async fn perf_spec_nonexistent_uuid() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfspecnonexist@example.com")
        .await;
    let org = server.create_org(&user, "Perf SpecNonExist Org").await;
    let project = server
        .create_project(&user, &org, "Perf SpecNonExist Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Query with a random UUID that doesn't exist as a spec
    let bogus_uuid = SpecUuid::new();
    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        &format!("&specs={bogus_uuid}"),
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Nonexistent spec UUID skips the testbed entirely, producing no results
    assert!(perf.results.is_empty());
}

// =============================================================================
// Section 12: Project UUID access
// =============================================================================

#[tokio::test]
async fn perf_access_by_project_uuid() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfprojuuid@example.com").await;
    let org = server.create_org(&user, "Perf ProjUuid Org").await;
    let project = server
        .create_project(&user, &org, "Perf ProjUuid Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // Use project UUID instead of slug
    let url = build_perf_url(
        &project.uuid.to_string(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

// =============================================================================
// Section: Parameter sets
// =============================================================================

fn create_parameter(
    server: &TestServer,
    benchmark_id: i32,
    set: &ParameterSet,
) -> (ParameterUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let parameter_uuid = ParameterUuid::new();
    diesel::insert_into(schema::parameter::table)
        .values((
            schema::parameter::uuid.eq(&parameter_uuid),
            schema::parameter::benchmark_id.eq(benchmark_id),
            schema::parameter::set.eq(set),
            schema::parameter::created.eq(&now),
            schema::parameter::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert parameter");
    let parameter_id: i32 = schema::parameter::table
        .filter(schema::parameter::uuid.eq(&parameter_uuid))
        .select(schema::parameter::id)
        .first(&mut conn)
        .expect("get parameter id");
    (parameter_uuid, parameter_id)
}

/// Add one variant to a benchmark inside the fixture's report.
fn create_variant(
    server: &TestServer,
    data: &PerfTestData,
    benchmark_id: i32,
    set: &str,
    value: f64,
) -> ParameterUuid {
    let set: ParameterSet = set.parse().expect("parse parameter set");
    let (parameter_uuid, parameter_id) = create_parameter(server, benchmark_id, &set);

    let mut conn = server.db_conn();
    let report_benchmark_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&report_benchmark_uuid),
            schema::report_benchmark::report_id.eq(data.report_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(benchmark_id),
            schema::report_benchmark::parameter_id.eq(parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert report_benchmark");
    let report_benchmark_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get report_benchmark id");

    create_metric(
        &mut conn,
        &MetricUuid::new(),
        report_benchmark_id,
        data.measure_id,
        value,
        None,
        None,
    );

    parameter_uuid
}

/// Create a second benchmark, with only the empty parameter set.
fn create_sibling_benchmark(server: &TestServer, project_id: i32) -> (BenchmarkUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let benchmark_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(&format!("test-benchmark-{benchmark_uuid}")),
            schema::benchmark::slug.eq(&format!("test-benchmark-{benchmark_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("get benchmark id");
    create_empty_parameter(&mut conn, benchmark_id);
    (benchmark_uuid, benchmark_id)
}

fn create_named_metric(
    server: &TestServer,
    report_benchmark_id: i32,
    measure_id: i32,
    name: MetricName,
    value: f64,
) {
    let mut conn = server.db_conn();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(MetricUuid::new()),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(measure_id),
            schema::metric::name.eq(name),
            schema::metric::value.eq(value),
        ))
        .execute(&mut conn)
        .expect("insert named metric");
}

/// The perf path for a fully built query, through the encoder a client uses.
fn perf_query_url(project_slug: &str, query: &JsonPerfQuery) -> String {
    format!(
        "/v0/projects/{project_slug}/perf?{}",
        query.to_query_string(&[]).expect("build query string")
    )
}

/// A `JsonPerfQuery` over the fixture's dimensions.
fn fixture_query(
    data: &PerfTestData,
    benchmarks: Vec<BenchmarkUuid>,
    parameters: &[&str],
) -> JsonPerfQuery {
    JsonPerfQuery {
        branches: vec![data.branch_uuid],
        heads: Vec::new(),
        testbeds: vec![data.testbed_uuid],
        specs: Vec::new(),
        benchmarks,
        parameters: parameters
            .iter()
            .map(|set| set.parse().expect("parse parameter set"))
            .collect(),
        measures: vec![data.measure_uuid],
        start_time: None,
        end_time: None,
    }
}

/// The `parameters` query value for filter blobs spelled exactly as given.
///
/// Each blob is percent encoded as a list element and then again as a query string
/// value, which are the two decodes the request undoes on the way in.
fn raw_parameters_query(blobs: &[&str]) -> String {
    blobs
        .iter()
        .map(|blob| {
            let element = blob
                .replace('%', "%25")
                .replace('"', "%22")
                .replace(',', "%2C");
            element.replace('%', "%25")
        })
        .collect::<Vec<_>>()
        .join("%2C")
}

async fn get_perf(server: &TestServer, token: &str, url: &str) -> JsonPerf {
    let resp = server
        .client
        .get(server.api_url(url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET {url}");
    resp.json().await.expect("parse response")
}

/// The canonical parameter set of every line, in response order.
fn line_parameters(perf: &JsonPerf) -> Vec<String> {
    perf.results
        .iter()
        .map(|result| result.parameter.set.canonical())
        .collect()
}

#[tokio::test]
async fn perf_fans_out_one_line_per_variant() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perffanout@example.com").await;
    let org = server.create_org(&user, "Perf Fan Out Org").await;
    let project = server
        .create_project(&user, &org, "Perf Fan Out Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    let sixteen = create_variant(&server, &data, data.benchmark_id, r#"{"size_mb": 16}"#, 1.0);
    let thirty_two = create_variant(&server, &data, data.benchmark_id, r#"{"size_mb": 32}"#, 2.0);

    let query = fixture_query(&data, vec![data.benchmark_uuid], &[]);
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;

    // The lines come out in variant creation order, so the empty set is first.
    assert_eq!(
        line_parameters(&perf),
        vec![
            "{}".to_owned(),
            r#"{"size_mb":16}"#.to_owned(),
            r#"{"size_mb":32}"#.to_owned()
        ]
    );
    for result in &perf.results {
        assert_eq!(result.benchmark.uuid, data.benchmark_uuid);
        assert_eq!(result.measure.uuid, data.measure_uuid);
        assert_eq!(result.parameter.benchmark, data.benchmark_uuid);
        assert_eq!(result.metrics.len(), 1, "one point per line");
    }
    assert_eq!(
        perf.results[0].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        42.0
    );
    assert_eq!(
        perf.results[1].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        1.0
    );
    assert_eq!(
        perf.results[2].metrics[0]
            .metric
            .expect("the metric triple")
            .value,
        2.0
    );
    assert_eq!(perf.results[1].parameter.uuid, sixteen);
    assert_eq!(perf.results[2].parameter.uuid, thirty_two);
}

#[tokio::test]
async fn perf_parameters_filter_is_an_or_of_ands() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perffilteror@example.com").await;
    let org = server.create_org(&user, "Perf Filter Or Org").await;
    let project = server
        .create_project(&user, &org, "Perf Filter Or Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    for (set, value) in [
        (r#"{"op": "read", "size_mb": 16}"#, 1.0),
        (r#"{"op": "read", "size_mb": 32}"#, 2.0),
        (r#"{"op": "write", "size_mb": 16}"#, 3.0),
        (r#"{"op": "write", "size_mb": 32}"#, 4.0),
    ] {
        create_variant(&server, &data, data.benchmark_id, set, value);
    }

    // Every read, plus the one write at 32.
    let query = fixture_query(
        &data,
        vec![data.benchmark_uuid],
        &[r#"{"op": "read"}"#, r#"{"op": "write", "size_mb": 32}"#],
    );
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;

    // A one key element matches every superset of itself.
    assert_eq!(
        line_parameters(&perf),
        vec![
            r#"{"op":"read","size_mb":16}"#.to_owned(),
            r#"{"op":"read","size_mb":32}"#.to_owned(),
            r#"{"op":"write","size_mb":32}"#.to_owned(),
        ]
    );

    // The empty element is a subset of every parameter set, so it matches them all.
    let query = fixture_query(&data, vec![data.benchmark_uuid], &["{}"]);
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;
    assert_eq!(perf.results.len(), 5, "the empty element matches every set");
}

#[tokio::test]
async fn perf_parameters_filter_matching_nothing_returns_no_lines() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perffilternone@example.com")
        .await;
    let org = server.create_org(&user, "Perf Filter None Org").await;
    let project = server
        .create_project(&user, &org, "Perf Filter None Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    create_variant(&server, &data, data.benchmark_id, r#"{"op": "read"}"#, 1.0);
    let (sibling_uuid, sibling_id) = create_sibling_benchmark(&server, project_id);
    create_variant(&server, &data, sibling_id, r#"{"op": "write"}"#, 2.0);

    // The write benchmark still returns its line while the read one returns none.
    let query = fixture_query(
        &data,
        vec![data.benchmark_uuid, sibling_uuid],
        &[r#"{"op": "write"}"#],
    );
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].benchmark.uuid, sibling_uuid);
    assert_eq!(
        perf.results[0].parameter.set.canonical(),
        r#"{"op":"write"}"#
    );

    // A filter that matches nothing anywhere returns nothing at all.
    let query = fixture_query(
        &data,
        vec![data.benchmark_uuid, sibling_uuid],
        &[r#"{"op": "trim"}"#],
    );
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_parameters_filter_is_number_spelling_blind() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perffilterspelling@example.com")
        .await;
    let org = server.create_org(&user, "Perf Filter Spelling Org").await;
    let project = server
        .create_project(&user, &org, "Perf Filter Spelling Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    create_variant(&server, &data, data.benchmark_id, r#"{"size_mb": 16}"#, 1.0);

    for blob in [
        r#"{"size_mb":16}"#,
        r#"{"size_mb":16.0}"#,
        r#"{"size_mb":1.6e1}"#,
    ] {
        let url = format!(
            "{}&parameters={}",
            build_perf_url(
                project.slug.as_ref(),
                &[data.branch_uuid],
                &[data.testbed_uuid],
                &[data.benchmark_uuid],
                &[data.measure_uuid],
                "",
            ),
            raw_parameters_query(&[blob])
        );
        let perf = get_perf(&server, &user.token, &url).await;
        assert_eq!(
            line_parameters(&perf),
            vec![r#"{"size_mb":16}"#.to_owned()],
            "{blob} must hit the same variant"
        );
    }

    // A different number is a different variant, spelling notwithstanding.
    let url = format!(
        "{}&parameters={}",
        build_perf_url(
            project.slug.as_ref(),
            &[data.branch_uuid],
            &[data.testbed_uuid],
            &[data.benchmark_uuid],
            &[data.measure_uuid],
            "",
        ),
        raw_parameters_query(&[r#"{"size_mb":16.5}"#])
    );
    let perf = get_perf(&server, &user.token, &url).await;
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_parameters_filter_empty_value_is_no_filter() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perffilterempty@example.com")
        .await;
    let org = server.create_org(&user, "Perf Filter Empty Org").await;
    let project = server
        .create_project(&user, &org, "Perf Filter Empty Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);
    create_variant(&server, &data, data.benchmark_id, r#"{"size_mb": 16}"#, 1.0);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "&parameters=",
    );
    let perf = get_perf(&server, &user.token, &url).await;
    assert_eq!(perf.results.len(), 2);
}

// =============================================================================
// Section: The metrics map
// =============================================================================

#[tokio::test]
async fn perf_metrics_map_carries_every_metric() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfmetricsmap@example.com")
        .await;
    let org = server.create_org(&user, "Perf Metrics Map Org").await;
    let project = server
        .create_project(&user, &org, "Perf Metrics Map Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let opts = PerfDataOptions {
        metric_value: 42.0,
        lower_value: Some(40.5),
        upper_value: Some(44.25),
        ..Default::default()
    };
    let data = create_perf_data_with_options(&server, project_id, &opts);
    create_named_metric(
        &server,
        data.report_benchmark_id,
        data.measure_id,
        "p99".parse().expect("parse metric name"),
        99.5,
    );
    let (_, boundary_id) = create_threshold_and_boundary(&server, &data, project_id);
    let alert_uuid = create_alert(&server, boundary_id);

    let query = fixture_query(&data, vec![data.benchmark_uuid], &[]);
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;

    assert_eq!(perf.results.len(), 1);
    let point = &perf.results[0].metrics[0];
    let names = point
        .metrics
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "lower_value".to_owned(),
            "p99".to_owned(),
            "upper_value".to_owned(),
            "value".to_owned()
        ]
    );
    for (name, value) in [
        ("value", 42.0),
        ("lower_value", 40.5),
        ("upper_value", 44.25),
        ("p99", 99.5),
    ] {
        let name = name.parse().expect("parse metric name");
        let entry = point.metrics.get(&name).expect("metric");
        assert_eq!(entry.value, value);
    }

    // Only the checked metric carries a boundaries list.
    let value = point
        .metrics
        .get(&MetricName::value())
        .expect("the value scalar");
    let boundaries = value
        .boundaries
        .as_ref()
        .expect("the value scalar is checked");
    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].boundary.baseline, Some(100.0.into()));
    assert_eq!(
        boundaries[0].alert.as_ref().map(|a| a.uuid),
        Some(alert_uuid)
    );
    for name in ["lower_value", "upper_value", "p99"] {
        let name = name.parse().expect("parse metric name");
        assert!(
            point
                .metrics
                .get(&name)
                .expect("metric")
                .boundaries
                .is_none(),
            "{name} is checked by nothing"
        );
    }

    // The deprecated singular fields still say what they always said.
    let metric = point.metric.expect("the metric triple");
    assert_eq!(metric.uuid, data.metric_uuid);
    assert_eq!(metric.value, 42.0);
    assert_eq!(metric.lower_value, Some(40.5.into()));
    assert_eq!(metric.upper_value, Some(44.25.into()));
    assert_eq!(
        point.threshold.as_ref().map(|t| t.uuid),
        boundaries.first().map(|b| b.threshold.uuid)
    );
    assert_eq!(
        point.boundary.and_then(|boundary| boundary.baseline),
        Some(100.0.into())
    );
    assert_eq!(point.alert.as_ref().map(|a| a.uuid), Some(alert_uuid));
}

#[tokio::test]
async fn perf_v0_response_fields_are_unchanged() {
    let server = perf_server().await;
    let user = server
        .signup("Test User", "perfbytecompat@example.com")
        .await;
    let org = server.create_org(&user, "Perf Byte Compat Org").await;
    let project = server
        .create_project(&user, &org, "Perf Byte Compat Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let opts = PerfDataOptions {
        metric_value: 42.0,
        lower_value: Some(40.5),
        upper_value: Some(44.25),
        ..Default::default()
    };
    let data = create_perf_data_with_options(&server, project_id, &opts);
    let (_, boundary_id) = create_threshold_and_boundary(&server, &data, project_id);
    let alert_uuid = create_alert(&server, boundary_id);

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &[data.measure_uuid],
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.expect("read response");

    // The float bytes are compared, not parsed values: parsing and re-serializing
    // would hide exactly the formatting drift this has to rule out.
    assert!(
        body.contains(&format!(
            r#""metric":{{"uuid":"{}","value":42.0,"lower_value":40.5,"upper_value":44.25}}"#,
            data.metric_uuid
        )),
        "{body}"
    );
    assert!(
        body.contains(r#""boundary":{"baseline":100.0,"lower_limit":50.0,"upper_limit":150.0}"#),
        "{body}"
    );

    let json: serde_json::Value = serde_json::from_str(&body).expect("parse response");
    let results = json["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    let result = results[0].as_object().expect("result");
    let mut keys = result.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "benchmark".to_owned(),
            "branch".to_owned(),
            "measure".to_owned(),
            "metrics".to_owned(),
            "parameter".to_owned(),
            "testbed".to_owned(),
        ],
        "the parameter set is the only field added beside the dimensions"
    );
    assert_eq!(result["branch"]["uuid"], data.branch_uuid.to_string());
    assert_eq!(result["testbed"]["uuid"], data.testbed_uuid.to_string());
    assert_eq!(result["benchmark"]["uuid"], data.benchmark_uuid.to_string());
    assert_eq!(result["measure"]["uuid"], data.measure_uuid.to_string());

    let points = result["metrics"].as_array().expect("metrics");
    assert_eq!(points.len(), 1);
    let point = points[0].as_object().expect("point");
    let mut keys = point.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "alert".to_owned(),
            "boundary".to_owned(),
            "end_time".to_owned(),
            "iteration".to_owned(),
            "metric".to_owned(),
            "metrics".to_owned(),
            "report".to_owned(),
            "start_time".to_owned(),
            "threshold".to_owned(),
            "version".to_owned(),
        ],
        "the metrics map is the only field added beside the deprecated ones"
    );
    assert_eq!(point["report"], data.report_uuid.to_string());
    assert_eq!(point["iteration"], 0);
    assert_eq!(point["version"]["number"], 1);
    assert_eq!(point["version"]["hash"], serde_json::Value::Null);
    assert_eq!(point["alert"]["uuid"], alert_uuid.to_string());
    assert_eq!(point["alert"]["limit"], "upper");
    assert_eq!(point["alert"]["status"], "active");
    assert!(point["threshold"]["model"].is_object());
}

#[tokio::test]
async fn perf_point_without_a_value_name_keeps_its_line() {
    let server = perf_server().await;
    let user = server.signup("Test User", "perfnovalue@example.com").await;
    let org = server.create_org(&user, "Perf No Value Org").await;
    let project = server
        .create_project(&user, &org, "Perf No Value Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // A second report benchmark, in its own iteration, naming only `p99`.
    let mut conn = server.db_conn();
    let report_benchmark_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&report_benchmark_uuid),
            schema::report_benchmark::report_id.eq(data.report_id),
            schema::report_benchmark::iteration.eq(1),
            schema::report_benchmark::benchmark_id.eq(data.benchmark_id),
            schema::report_benchmark::parameter_id.eq(data.parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert report_benchmark");
    let report_benchmark_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get report_benchmark id");
    drop(conn);
    create_named_metric(
        &server,
        report_benchmark_id,
        data.measure_id,
        "p99".parse().expect("parse metric name"),
        99.5,
    );

    let query = fixture_query(&data, vec![data.benchmark_uuid], &[]);
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(project.slug.as_ref(), &query),
    )
    .await;

    assert_eq!(perf.results.len(), 1, "one variant, one line");
    let points = &perf.results[0].metrics;
    assert_eq!(
        points.len(),
        2,
        "the point that named no value is still a point"
    );

    // The fixture's point still says everything it always said.
    let value_point = &points[0];
    assert_eq!(value_point.iteration.0, 0);
    assert_eq!(
        value_point.metric.expect("the metric triple").value,
        42.0,
        "a point that names value still carries the triple"
    );

    // `p99` is in the map and nothing is in the deprecated fields.
    let named_point = &points[1];
    assert_eq!(named_point.iteration.0, 1);
    assert!(
        named_point.metric.is_none(),
        "a point that names no value carries no triple"
    );
    assert!(named_point.threshold.is_none());
    assert!(named_point.boundary.is_none());
    assert!(named_point.alert.is_none());
    let names = named_point
        .metrics
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["p99".to_owned()]);
    let p99 = named_point
        .metrics
        .get(&"p99".parse().expect("parse metric name"))
        .expect("the p99 scalar");
    assert_eq!(p99.value, 99.5);
    assert!(p99.boundaries.is_none());
}

// =============================================================================
// Section: The permutation grid
// =============================================================================

/// A grid of perf dimensions, with a report for every (branch, testbed) pair and
/// a metric for every cell of it.
///
/// One `create_perf_data` fixture is one permutation, so it can only pin the first
/// line of a response. A grid pins every line.
struct PerfGrid {
    project_id: i32,
    /// The branches, and the head each one is currently on.
    branches: Vec<(BranchUuid, i32)>,
    heads: Vec<(HeadUuid, i32)>,
    testbeds: Vec<(TestbedUuid, i32)>,
    benchmarks: Vec<(BenchmarkUuid, i32)>,
    measures: Vec<(MeasureUuid, i32)>,
    /// Per benchmark, its variants in creation order, the empty set first.
    variants: Vec<Vec<(i32, String)>>,
    /// The report of one (branch, testbed) pair, for the pairs that reported.
    reports: BTreeMap<(usize, usize), i32>,
    /// The report benchmark of one (branch, testbed, benchmark, variant) cell.
    report_benchmarks: BTreeMap<(usize, usize, usize, usize), i32>,
    /// The `value` row of one (branch, testbed, benchmark, variant, measure) cell.
    metrics: BTreeMap<(usize, usize, usize, usize, usize), MetricUuid>,
}

/// The value of one cell, distinct for every cell of the grid so that a line's
/// points name the cell they were read from.
fn grid_value(
    branch: usize,
    testbed: usize,
    benchmark: usize,
    variant: usize,
    measure: usize,
) -> f64 {
    let cell = 10_000 * branch + 1_000 * testbed + 100 * benchmark + 10 * variant + measure;
    f64::from(u16::try_from(cell).expect("the grid cell fits"))
}

impl PerfGrid {
    /// Build the dimensions of the grid without any report at all.
    fn empty(
        server: &TestServer,
        project_id: i32,
        branches: usize,
        testbeds: usize,
        benchmarks: usize,
        measures: usize,
    ) -> Self {
        let mut grid = Self {
            project_id,
            branches: Vec::new(),
            heads: Vec::new(),
            testbeds: Vec::new(),
            benchmarks: Vec::new(),
            measures: Vec::new(),
            variants: Vec::new(),
            reports: BTreeMap::new(),
            report_benchmarks: BTreeMap::new(),
            metrics: BTreeMap::new(),
        };
        for branch in 0..branches {
            let (branch_uuid, branch_id, head_uuid, head_id) =
                insert_branch(server, project_id, branch);
            grid.branches.push((branch_uuid, branch_id));
            grid.heads.push((head_uuid, head_id));
        }
        for testbed in 0..testbeds {
            grid.testbeds
                .push(insert_testbed(server, project_id, testbed));
        }
        for benchmark in 0..benchmarks {
            let (benchmark_uuid, benchmark_id, parameter_id) =
                insert_benchmark(server, project_id, benchmark);
            grid.benchmarks.push((benchmark_uuid, benchmark_id));
            grid.variants.push(vec![(parameter_id, "{}".to_owned())]);
        }
        for measure in 0..measures {
            grid.measures
                .push(insert_measure(server, project_id, measure));
        }
        grid
    }

    /// Report one (branch, testbed) pair over every cell of the grid.
    fn report(&mut self, server: &TestServer, branch: usize, testbed: usize) {
        let benchmarks = (0..self.benchmarks.len()).collect::<Vec<_>>();
        self.report_some(server, branch, testbed, &benchmarks);
    }

    /// Report one (branch, testbed) pair for the named benchmarks only, so the
    /// benchmarks left out have dimensions and no rows.
    fn report_some(
        &mut self,
        server: &TestServer,
        branch: usize,
        testbed: usize,
        benchmarks: &[usize],
    ) {
        let (_, head_id) = self.heads[branch];
        let (_, testbed_id) = self.testbeds[testbed];
        let version_id = insert_version(server, self.project_id, head_id);
        let report_id = insert_report(server, self.project_id, head_id, version_id, testbed_id);
        self.reports.insert((branch, testbed), report_id);
        for benchmark in benchmarks.iter().copied() {
            for variant in 0..self.variants[benchmark].len() {
                self.report_variant(server, branch, testbed, benchmark, variant);
            }
        }
    }

    /// Add one variant to a benchmark, wherever that benchmark already reports.
    fn add_variant(&mut self, server: &TestServer, benchmark: usize, set: &str) -> ParameterUuid {
        let (_, benchmark_id) = self.benchmarks[benchmark];
        let parsed: ParameterSet = set.parse().expect("parse parameter set");
        let (parameter_uuid, parameter_id) = create_parameter(server, benchmark_id, &parsed);
        self.variants[benchmark].push((parameter_id, parsed.canonical()));
        let variant = self.variants[benchmark].len() - 1;
        let reported = self
            .reports
            .keys()
            .copied()
            .filter(|(branch, testbed)| {
                self.report_benchmarks
                    .contains_key(&(*branch, *testbed, benchmark, 0))
            })
            .collect::<Vec<_>>();
        for (branch, testbed) in reported {
            self.report_variant(server, branch, testbed, benchmark, variant);
        }
        parameter_uuid
    }

    /// Report one cell: its report benchmark, and one metric per measure.
    fn report_variant(
        &mut self,
        server: &TestServer,
        branch: usize,
        testbed: usize,
        benchmark: usize,
        variant: usize,
    ) {
        let Some(report_id) = self.reports.get(&(branch, testbed)).copied() else {
            return;
        };
        let (_, benchmark_id) = self.benchmarks[benchmark];
        let (parameter_id, _) = self.variants[benchmark][variant];
        let report_benchmark_id =
            insert_report_benchmark(server, report_id, benchmark_id, parameter_id);
        self.report_benchmarks
            .insert((branch, testbed, benchmark, variant), report_benchmark_id);
        let mut conn = server.db_conn();
        for (measure, (_, measure_id)) in self.measures.iter().enumerate() {
            let metric_uuid = MetricUuid::new();
            create_metric(
                &mut conn,
                &metric_uuid,
                report_benchmark_id,
                *measure_id,
                grid_value(branch, testbed, benchmark, variant, measure),
                None,
                None,
            );
            self.metrics
                .insert((branch, testbed, benchmark, variant, measure), metric_uuid);
        }
    }

    /// The query over every dimension of the grid.
    fn query(&self) -> JsonPerfQuery {
        JsonPerfQuery {
            branches: self.branches.iter().map(|(uuid, _)| *uuid).collect(),
            heads: Vec::new(),
            testbeds: self.testbeds.iter().map(|(uuid, _)| *uuid).collect(),
            specs: Vec::new(),
            benchmarks: self.benchmarks.iter().map(|(uuid, _)| *uuid).collect(),
            parameters: Vec::new(),
            measures: self.measures.iter().map(|(uuid, _)| *uuid).collect(),
            start_time: None,
            end_time: None,
        }
    }

    /// Every line the grid's own query returns, in the order the endpoint walks
    /// its dimensions: branch, testbed, benchmark, variant, measure.
    fn expected_lines(&self) -> Vec<GridLine> {
        let mut lines = Vec::new();
        for branch in 0..self.branches.len() {
            for testbed in 0..self.testbeds.len() {
                for benchmark in 0..self.benchmarks.len() {
                    for measure in 0..self.measures.len() {
                        for variant in 0..self.variants[benchmark].len() {
                            if !self
                                .metrics
                                .contains_key(&(branch, testbed, benchmark, variant, measure))
                            {
                                continue;
                            }
                            lines.push(GridLine {
                                branch: self.branches[branch].0,
                                testbed: self.testbeds[testbed].0,
                                benchmark: self.benchmarks[benchmark].0,
                                parameter: self.variants[benchmark][variant].1.clone(),
                                measure: self.measures[measure].0,
                                value: grid_value(branch, testbed, benchmark, variant, measure),
                            });
                        }
                    }
                }
            }
        }
        lines
    }
}

/// One line of a perf response, flattened to what a grid can predict.
#[derive(Debug, PartialEq)]
struct GridLine {
    branch: BranchUuid,
    testbed: TestbedUuid,
    benchmark: BenchmarkUuid,
    parameter: String,
    measure: MeasureUuid,
    value: f64,
}

/// The lines of a response, flattened the same way. Every line of a grid holds
/// exactly one point, so its value names the cell the line was read from.
fn response_lines(perf: &JsonPerf) -> Vec<GridLine> {
    perf.results
        .iter()
        .map(|result| {
            assert_eq!(result.metrics.len(), 1, "one report, so one point per line");
            GridLine {
                branch: result.branch.uuid,
                testbed: result.testbed.uuid,
                benchmark: result.benchmark.uuid,
                parameter: result.parameter.set.canonical(),
                measure: result.measure.uuid,
                value: result.metrics[0]
                    .metrics
                    .get(&MetricName::value())
                    .expect("the value metric")
                    .value
                    .into_inner(),
            }
        })
        .collect()
}

/// Insert a branch, its head, and point the branch at that head.
fn insert_branch(
    server: &TestServer,
    project_id: i32,
    index: usize,
) -> (BranchUuid, i32, HeadUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let branch_uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(&branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq(&format!("grid-branch-{index}-{branch_uuid}")),
            schema::branch::slug.eq(&format!("grid-branch-{index}-{branch_uuid}")),
            schema::branch::created.eq(&now),
            schema::branch::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert branch");
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(&branch_uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("get branch id");

    let head_uuid = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(&head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert head");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(&head_uuid))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("get head id");

    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(&mut conn)
        .expect("update branch head_id");

    (branch_uuid, branch_id, head_uuid, head_id)
}

fn insert_testbed(server: &TestServer, project_id: i32, index: usize) -> (TestbedUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let testbed_uuid = TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(&format!("grid-testbed-{index}-{testbed_uuid}")),
            schema::testbed::slug.eq(&format!("grid-testbed-{index}-{testbed_uuid}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert testbed");
    let testbed_id: i32 = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&testbed_uuid))
        .select(schema::testbed::id)
        .first(&mut conn)
        .expect("get testbed id");
    (testbed_uuid, testbed_id)
}

fn insert_measure(server: &TestServer, project_id: i32, index: usize) -> (MeasureUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let measure_uuid = MeasureUuid::new();
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(&format!("grid-measure-{index}-{measure_uuid}")),
            schema::measure::slug.eq(&format!("grid-measure-{index}-{measure_uuid}")),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert measure");
    let measure_id: i32 = schema::measure::table
        .filter(schema::measure::uuid.eq(&measure_uuid))
        .select(schema::measure::id)
        .first(&mut conn)
        .expect("get measure id");
    (measure_uuid, measure_id)
}

/// Insert a benchmark and the empty parameter set it is born with.
fn insert_benchmark(
    server: &TestServer,
    project_id: i32,
    index: usize,
) -> (BenchmarkUuid, i32, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let benchmark_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(&format!("grid-benchmark-{index}-{benchmark_uuid}")),
            schema::benchmark::slug.eq(&format!("grid-benchmark-{index}-{benchmark_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("get benchmark id");
    let parameter_id = create_empty_parameter(&mut conn, benchmark_id);
    (benchmark_uuid, benchmark_id, parameter_id)
}

fn insert_version(server: &TestServer, project_id: i32, head_id: i32) -> i32 {
    let mut conn = server.db_conn();
    let version_uuid = VersionUuid::new();
    let number: i32 = schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("count head versions")
        .try_into()
        .expect("version number fits");
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(number + 1),
        ))
        .execute(&mut conn)
        .expect("insert version");
    let version_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("get version id");
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut conn)
        .expect("insert head_version");
    version_id
}

fn insert_report(
    server: &TestServer,
    project_id: i32,
    head_id: i32,
    version_id: i32,
    testbed_id: i32,
) -> i32 {
    let mut conn = server.db_conn();
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
        .expect("insert report");
    schema::report::table
        .filter(schema::report::uuid.eq(&report_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("get report id")
}

fn insert_report_benchmark(
    server: &TestServer,
    report_id: i32,
    benchmark_id: i32,
    parameter_id: i32,
) -> i32 {
    let mut conn = server.db_conn();
    let report_benchmark_uuid = ReportBenchmarkUuid::new();
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(&report_benchmark_uuid),
            schema::report_benchmark::report_id.eq(report_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(benchmark_id),
            schema::report_benchmark::parameter_id.eq(parameter_id),
        ))
        .execute(&mut conn)
        .expect("insert report_benchmark");
    schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("get report_benchmark id")
}

/// Create a threshold, its model, and a boundary on one metric, and return the
/// threshold's UUID with the boundary's row.
fn create_check(
    server: &TestServer,
    project_id: i32,
    branch_id: i32,
    testbed_id: i32,
    measure_id: i32,
    metric_uuid: MetricUuid,
) -> (bencher_json::ThresholdUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();

    let threshold_uuid = bencher_json::ThresholdUuid::new();
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(&threshold_uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch_id),
            schema::threshold::testbed_id.eq(testbed_id),
            schema::threshold::measure_id.eq(measure_id),
            schema::threshold::created.eq(&now),
            schema::threshold::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert threshold");
    let threshold_id: i32 = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&threshold_uuid))
        .select(schema::threshold::id)
        .first(&mut conn)
        .expect("get threshold id");

    let model_uuid = bencher_json::ModelUuid::new();
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(&model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(0),
            schema::model::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert model");
    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(&model_uuid))
        .select(schema::model::id)
        .first(&mut conn)
        .expect("get model id");
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(&mut conn)
        .expect("update threshold model_id");

    let metric_id: i32 = schema::metric::table
        .filter(schema::metric::uuid.eq(&metric_uuid))
        .select(schema::metric::id)
        .first(&mut conn)
        .expect("get metric id");
    let boundary_uuid = BoundaryUuid::new();
    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(&boundary_uuid),
            schema::boundary::metric_id.eq(metric_id),
            schema::boundary::threshold_id.eq(threshold_id),
            schema::boundary::model_id.eq(model_id),
            schema::boundary::baseline.eq(Some(100.0)),
            schema::boundary::lower_limit.eq(Some(50.0)),
            schema::boundary::upper_limit.eq(Some(150.0)),
        ))
        .execute(&mut conn)
        .expect("insert boundary");
    let boundary_id: i32 = schema::boundary::table
        .filter(schema::boundary::uuid.eq(&boundary_uuid))
        .select(schema::boundary::id)
        .first(&mut conn)
        .expect("get boundary id");

    (threshold_uuid, boundary_id)
}

/// Set up a project with the dimensions of a grid and no report at all.
async fn empty_grid_project(
    server: &TestServer,
    label: &str,
    branches: usize,
    testbeds: usize,
    benchmarks: usize,
    measures: usize,
) -> (TestUser, String, PerfGrid) {
    let user = server
        .signup("Test User", &format!("perfgrid{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Perf Grid {label} Org"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Perf Grid {label} Project"))
        .await;
    let project_slug: String = AsRef::<str>::as_ref(&project.slug).to_owned();
    let project_id = get_project_id(server, &project_slug);
    let grid = PerfGrid::empty(server, project_id, branches, testbeds, benchmarks, measures);
    (user, project_slug, grid)
}

/// Set up a project with a grid over it, every pair reported.
async fn grid_project(
    server: &TestServer,
    label: &str,
    branches: usize,
    testbeds: usize,
    benchmarks: usize,
    measures: usize,
) -> (TestUser, String, PerfGrid) {
    let (user, project_slug, mut grid) =
        empty_grid_project(server, label, branches, testbeds, benchmarks, measures).await;
    for branch in 0..branches {
        for testbed in 0..testbeds {
            grid.report(server, branch, testbed);
        }
    }
    (user, project_slug, grid)
}

#[tokio::test]
async fn perf_every_line_carries_its_own_dimensions() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = grid_project(&server, "dims", 2, 2, 2, 2).await;
    // One benchmark fans out over two variants, so a permutation is not a line.
    grid.add_variant(&server, 0, r#"{"size_mb": 16}"#);

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;

    // `2` branches, `2` testbeds, `2` benchmarks, and `2` measures is `16`
    // permutations, and the benchmark with `2` variants makes `24` lines.
    assert_eq!(perf.results.len(), 24);
    assert_eq!(response_lines(&perf), grid.expected_lines());
}

#[tokio::test]
async fn perf_permutation_without_metrics_returns_no_line() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = empty_grid_project(&server, "empty", 2, 2, 2, 1).await;

    // Two branches and two testbeds, but only one pair ever reported, and that
    // report holds only the first benchmark.
    grid.report_some(&server, 0, 0, &[0]);
    // The second benchmark has variants of its own and no report of any of them.
    grid.add_variant(&server, 1, r#"{"size_mb": 16}"#);
    grid.add_variant(&server, 1, r#"{"size_mb": 32}"#);

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;

    // Only the pair that reported, and only the benchmark it reported.
    assert_eq!(response_lines(&perf), grid.expected_lines());
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].branch.uuid, grid.branches[0].0);
    assert_eq!(perf.results[0].testbed.uuid, grid.testbeds[0].0);
    assert_eq!(perf.results[0].benchmark.uuid, grid.benchmarks[0].0);
}

#[tokio::test]
async fn perf_unknown_dimension_uuids_skip_their_permutations() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "unknown", 1, 1, 1, 1).await;

    let mut query = grid.query();
    query.branches.push(BranchUuid::new());
    query.testbeds.push(TestbedUuid::new());
    query.benchmarks.push(BenchmarkUuid::new());
    query.measures.push(MeasureUuid::new());
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert_eq!(response_lines(&perf), grid.expected_lines());

    // A head that names nothing skips its branch the same way.
    let mut query = grid.query();
    query.heads = vec![Some(HeadUuid::new())];
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_checks_land_on_the_lines_they_checked() {
    let server = perf_server().await;
    let (user, project_slug, mut grid) = grid_project(&server, "checks", 2, 1, 1, 2).await;
    grid.add_variant(&server, 0, r#"{"size_mb": 16}"#);

    // One threshold, on the first branch and the first measure.
    let checked = grid.metrics[&(0, 0, 0, 0, 0)];
    let (threshold_uuid, boundary_id) = create_check(
        &server,
        grid.project_id,
        grid.branches[0].1,
        grid.testbeds[0].1,
        grid.measures[0].1,
        checked,
    );
    let alert_uuid = create_alert(&server, boundary_id);

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    assert_eq!(response_lines(&perf), grid.expected_lines());

    let mut checked_lines = 0;
    for result in &perf.results {
        let point = &result.metrics[0];
        let value = point
            .metrics
            .get(&MetricName::value())
            .expect("the value metric");
        let is_checked = result.branch.uuid == grid.branches[0].0
            && result.measure.uuid == grid.measures[0].0
            && result.parameter.set.canonical() == "{}";
        if is_checked {
            checked_lines += 1;
            let boundaries = value.boundaries.as_ref().expect("the checked metric");
            assert_eq!(boundaries.len(), 1);
            assert_eq!(boundaries[0].threshold.uuid, threshold_uuid);
            assert_eq!(boundaries[0].boundary.baseline, Some(100.0.into()));
            assert_eq!(
                boundaries[0].alert.as_ref().map(|alert| alert.uuid),
                Some(alert_uuid)
            );
            // The deprecated singular fields say the same thing.
            assert_eq!(
                point.threshold.as_ref().map(|t| t.uuid),
                Some(threshold_uuid)
            );
            assert_eq!(
                point.boundary.and_then(|boundary| boundary.baseline),
                Some(100.0.into())
            );
            assert_eq!(
                point.alert.as_ref().map(|alert| alert.uuid),
                Some(alert_uuid)
            );
        } else {
            assert!(value.boundaries.is_none(), "nothing checked this metric");
            assert!(point.threshold.is_none());
            assert!(point.boundary.is_none());
            assert!(point.alert.is_none());
        }
    }
    assert_eq!(checked_lines, 1, "one line was checked");
}

#[tokio::test]
async fn perf_v0_metric_triple_on_every_line() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "triple", 2, 1, 1, 1).await;

    // Give both lines the bounds the deprecated triple carries.
    for branch in 0..2 {
        let report_benchmark_id = grid.report_benchmarks[&(branch, 0, 0, 0)];
        let value = grid_value(branch, 0, 0, 0, 0);
        create_named_metric(
            &server,
            report_benchmark_id,
            grid.measures[0].1,
            MetricName::lower_value(),
            value - 0.5,
        );
        create_named_metric(
            &server,
            report_benchmark_id,
            grid.measures[0].1,
            MetricName::upper_value(),
            value + 0.25,
        );
    }

    let url = perf_query_url(&project_slug, &grid.query());
    let resp = server
        .client
        .get(server.api_url(&url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.expect("read response");

    // The float bytes are compared, not parsed values, on every line.
    for branch in 0..2 {
        let value = grid_value(branch, 0, 0, 0, 0);
        let metric_uuid = grid.metrics[&(branch, 0, 0, 0, 0)];
        assert!(
            body.contains(&format!(
                r#""metric":{{"uuid":"{metric_uuid}","value":{value:?},"lower_value":{:?},"upper_value":{:?}}}"#,
                value - 0.5,
                value + 0.25
            )),
            "{body}"
        );
    }

    let json: serde_json::Value = serde_json::from_str(&body).expect("parse response");
    let results = json["results"].as_array().expect("results");
    assert_eq!(results.len(), 2);
    for (branch, result) in results.iter().enumerate() {
        assert_eq!(
            result["branch"]["uuid"],
            grid.branches[branch].0.to_string()
        );
        let points = result["metrics"].as_array().expect("metrics");
        assert_eq!(points.len(), 1);
        let mut keys = points[0]
            .as_object()
            .expect("point")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "alert".to_owned(),
                "boundary".to_owned(),
                "end_time".to_owned(),
                "iteration".to_owned(),
                "metric".to_owned(),
                "metrics".to_owned(),
                "report".to_owned(),
                "start_time".to_owned(),
                "threshold".to_owned(),
                "version".to_owned(),
            ],
        );
    }
}

// =============================================================================
// Section: The default window
// =============================================================================

/// A grid whose one report sits at the given offset in seconds from the server's
/// frozen now, so a test can put a report inside or outside the default window.
async fn window_grid(
    server: &TestServer,
    label: &str,
    offset_secs: i64,
) -> (TestUser, String, PerfGrid) {
    let (user, project_slug, grid) = grid_project(server, label, 1, 1, 1, 1).await;
    let start_time = bencher_json::DateTime::try_from(base_timestamp().timestamp() + offset_secs)
        .expect("valid timestamp");
    let mut conn = server.db_conn();
    diesel::update(schema::report::table)
        .set((
            schema::report::start_time.eq(&start_time),
            schema::report::end_time.eq(&start_time),
        ))
        .execute(&mut conn)
        .expect("move the report in time");
    (user, project_slug, grid)
}

// 4 weeks, the history every one of its four sites reaches back.
const REPORT_HISTORY_SECS: i64 = 60 * 60 * 24 * 28;

#[tokio::test]
async fn perf_defaults_to_the_report_history() {
    let server = perf_server().await;
    let (user, project_slug, grid) =
        window_grid(&server, "windowin", -REPORT_HISTORY_SECS + 60).await;
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    assert_eq!(response_lines(&perf), grid.expected_lines());

    let server = perf_server().await;
    let (user, project_slug, grid) =
        window_grid(&server, "windowout", -REPORT_HISTORY_SECS - 60).await;
    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    assert!(
        perf.results.is_empty(),
        "a report older than the window is outside it"
    );
}

#[tokio::test]
async fn perf_echoes_the_default_window() {
    let server = perf_server().await;
    let (user, project_slug, grid) = grid_project(&server, "windowecho", 1, 1, 1, 1).await;

    let perf = get_perf(
        &server,
        &user.token,
        &perf_query_url(&project_slug, &grid.query()),
    )
    .await;
    assert_eq!(response_lines(&perf), grid.expected_lines());
    assert_eq!(
        perf.start_time.expect("the window it plotted").timestamp(),
        base_timestamp().timestamp() - REPORT_HISTORY_SECS,
        "the window runs back from the server's now"
    );
    assert!(perf.end_time.is_none(), "the query named no end time");

    // With an end time the window runs back from that instead.
    let mut query = grid.query();
    let end_time = bencher_json::DateTime::try_from(base_timestamp().timestamp() - 60)
        .expect("valid timestamp");
    query.end_time = Some(end_time);
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert_eq!(
        perf.start_time.expect("the window it plotted").timestamp(),
        end_time.timestamp() - REPORT_HISTORY_SECS
    );
}

#[tokio::test]
async fn perf_honors_an_explicit_start_time_however_old() {
    let server = perf_server().await;
    let (user, project_slug, grid) =
        window_grid(&server, "windowold", -REPORT_HISTORY_SECS * 10).await;

    let mut query = grid.query();
    query.start_time = Some(
        bencher_json::DateTime::try_from(base_timestamp().timestamp() - REPORT_HISTORY_SECS * 20)
            .expect("valid timestamp"),
    );
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert_eq!(response_lines(&perf), grid.expected_lines());
    assert_eq!(perf.start_time, query.start_time);
}

#[tokio::test]
async fn perf_window_runs_back_from_the_end_time() {
    // The report sits ten windows back, which is outside the window from now and
    // inside the window from an end time just after it.
    let server = perf_server().await;
    let (user, project_slug, grid) =
        window_grid(&server, "windowend", -REPORT_HISTORY_SECS * 10).await;
    let report_time = base_timestamp().timestamp() - REPORT_HISTORY_SECS * 10;

    let mut query = grid.query();
    query.end_time =
        Some(bencher_json::DateTime::try_from(report_time + 60).expect("valid timestamp"));
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert_eq!(response_lines(&perf), grid.expected_lines());

    // One window further back and the same report falls outside it.
    let mut query = grid.query();
    query.end_time = Some(
        bencher_json::DateTime::try_from(report_time + REPORT_HISTORY_SECS + 60)
            .expect("valid timestamp"),
    );
    let perf = get_perf(&server, &user.token, &perf_query_url(&project_slug, &query)).await;
    assert!(perf.results.is_empty());
}
