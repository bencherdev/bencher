#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::indexing_slicing
)]
//! Integration tests for the `/v0/projects/{project}/perf` endpoint.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, get_project_id},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, HeadUuid, JobPriority, JobStatus, JobUuid,
    JsonPerf, MeasureUuid, MetricUuid, ReportBenchmarkUuid, ReportUuid, SpecUuid, TestbedUuid,
    VersionUuid,
    project::{alert::AlertStatus, boundary::BoundaryLimit},
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

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
    if opts.lower_value.is_some() || opts.upper_value.is_some() {
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(&metric_uuid),
                schema::metric::report_benchmark_id.eq(report_benchmark_id),
                schema::metric::measure_id.eq(measure_id),
                schema::metric::value.eq(opts.metric_value),
                schema::metric::lower_value.eq(opts.lower_value),
                schema::metric::upper_value.eq(opts.upper_value),
            ))
            .execute(&mut conn)
            .expect("insert metric with bounds");
    } else {
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(&metric_uuid),
                schema::metric::report_benchmark_id.eq(report_benchmark_id),
                schema::metric::measure_id.eq(measure_id),
                schema::metric::value.eq(opts.metric_value),
            ))
            .execute(&mut conn)
            .expect("insert metric");
    }

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
            schema::job::priority.eq(JobPriority::default()),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("insert job");
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 1);
    assert_eq!(perf.results[0].metrics[0].metric.value, 42.0);
    assert_eq!(perf.results[0].metrics[0].report, data.report_uuid);
}

#[tokio::test]
async fn perf_get_response_structure() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Verify top-level structure
    assert_eq!(perf.project.uuid, project.uuid);
    assert!(perf.start_time.is_none());
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_get_multiple_metrics_same_permutation() {
    let server = TestServer::new().await;
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
            schema::metric::measure_id.eq(data.measure_id),
            schema::metric::value.eq(99.0),
        ))
        .execute(&mut conn)
        .expect("insert metric2");

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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Ordered by version number (oldest first)
    assert_eq!(perf.results[0].metrics[0].metric.value, 42.0);
    assert_eq!(perf.results[0].metrics[1].metric.value, 99.0);
}

// =============================================================================
// Section 2: Query filtering
// =============================================================================

#[tokio::test]
async fn perf_filter_by_start_time() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_filter_includes_matching_time() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&metric2_uuid),
            schema::metric::report_benchmark_id.eq(data.report_benchmark_id),
            schema::metric::measure_id.eq(measure2_id),
            schema::metric::value.eq(1024.0),
        ))
        .execute(&mut conn)
        .expect("insert metric2");

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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_testbeds_param() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_benchmarks_param() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_missing_measures_param() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_invalid_branch_uuid() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_nonexistent_project() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "perfnoproj@example.com").await;

    let uuid = BranchUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/nonexistent-project/perf?branches={uuid}&testbeds={uuid}&benchmarks={uuid}&measures={uuid}"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn perf_no_query_params() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "perfnoparams@example.com").await;
    let org = server.create_org(&user, "Perf NoParams Org").await;
    let project = server
        .create_project(&user, &org, "Perf NoParams Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/perf")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn perf_empty_branches_value() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
    let server = TestServer::new().await;
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

#[tokio::test]
async fn perf_private_project_wrong_user() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&other.token))
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
// Section 5: Permutation limit
// =============================================================================

#[tokio::test]
async fn perf_permutation_limit_exact_255() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "perflimit255@example.com").await;
    let org = server.create_org(&user, "Perf Limit255 Org").await;
    let project = server
        .create_project(&user, &org, "Perf Limit255 Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // 1 branch * 1 testbed * 1 benchmark * 255 measures = 255 permutations
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let mut measure_uuids = vec![data.measure_uuid];
    for i in 0..254 {
        let m_uuid = MeasureUuid::new();
        diesel::insert_into(schema::measure::table)
            .values((
                schema::measure::uuid.eq(&m_uuid),
                schema::measure::project_id.eq(project_id),
                schema::measure::name.eq(&format!("measure-{i}")),
                schema::measure::slug.eq(&format!("measure-{i}-{m_uuid}")),
                schema::measure::units.eq("ns"),
                schema::measure::created.eq(&now),
                schema::measure::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("insert measure");
        let m_id: i32 = schema::measure::table
            .filter(schema::measure::uuid.eq(&m_uuid))
            .select(schema::measure::id)
            .first(&mut conn)
            .expect("get measure id");

        let metric_uuid = MetricUuid::new();
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(&metric_uuid),
                schema::metric::report_benchmark_id.eq(data.report_benchmark_id),
                schema::metric::measure_id.eq(m_id),
                schema::metric::value.eq(1.0),
            ))
            .execute(&mut conn)
            .expect("insert metric");
        measure_uuids.push(m_uuid);
    }

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &measure_uuids,
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 255);
}

#[tokio::test]
async fn perf_permutation_limit_truncated_256() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "perflimit256@example.com").await;
    let org = server.create_org(&user, "Perf Limit256 Org").await;
    let project = server
        .create_project(&user, &org, "Perf Limit256 Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let data = create_perf_data(&server, project_id);

    // 1 branch * 1 testbed * 1 benchmark * 256 measures = 256 permutations
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let mut measure_uuids = vec![data.measure_uuid];
    for i in 0..255 {
        let m_uuid = MeasureUuid::new();
        diesel::insert_into(schema::measure::table)
            .values((
                schema::measure::uuid.eq(&m_uuid),
                schema::measure::project_id.eq(project_id),
                schema::measure::name.eq(&format!("measure-t-{i}")),
                schema::measure::slug.eq(&format!("measure-t-{i}-{m_uuid}")),
                schema::measure::units.eq("ns"),
                schema::measure::created.eq(&now),
                schema::measure::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("insert measure");
        let m_id: i32 = schema::measure::table
            .filter(schema::measure::uuid.eq(&m_uuid))
            .select(schema::measure::id)
            .first(&mut conn)
            .expect("get measure id");

        let metric_uuid = MetricUuid::new();
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(&metric_uuid),
                schema::metric::report_benchmark_id.eq(data.report_benchmark_id),
                schema::metric::measure_id.eq(m_id),
                schema::metric::value.eq(1.0),
            ))
            .execute(&mut conn)
            .expect("insert metric");
        measure_uuids.push(m_uuid);
    }

    let url = build_perf_url(
        project.slug.as_ref(),
        &[data.branch_uuid],
        &[data.testbed_uuid],
        &[data.benchmark_uuid],
        &measure_uuids,
        "",
    );
    let resp = server
        .client
        .get(server.api_url(&url))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // Should be truncated to 255
    assert_eq!(perf.results.len(), 255);
}

// =============================================================================
// Section 6: Threshold / boundary / alert
// =============================================================================

#[tokio::test]
async fn perf_without_threshold() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // v1 should come first (oldest version number)
    assert_eq!(perf.results[0].metrics[0].version.number.0, 1);
    assert_eq!(perf.results[0].metrics[0].metric.value, 100.0);
    assert_eq!(perf.results[0].metrics[1].version.number.0, 2);
    assert_eq!(perf.results[0].metrics[1].metric.value, 200.0);
}

#[tokio::test]
async fn perf_ordered_by_start_time_within_version() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Earlier start_time should come first (within same version number)
    assert_eq!(perf.results[0].metrics[0].metric.value, 100.0);
    assert_eq!(perf.results[0].metrics[1].metric.value, 200.0);
}

#[tokio::test]
async fn perf_version_hash_returned() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
            .unwrap()
            .as_ref()
            .starts_with("abc1234")
    );
}

// =============================================================================
// Section 8: Branch head
// =============================================================================

#[tokio::test]
async fn perf_default_head() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}

#[tokio::test]
async fn perf_explicit_head() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_wrong_project_benchmark() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert!(perf.results.is_empty());
}

#[tokio::test]
async fn perf_wrong_project_measure() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    let metric = &perf.results[0].metrics[0].metric;
    assert_eq!(metric.value, 100.0);
    assert_eq!(metric.lower_value, Some(90.0.into()));
    assert_eq!(metric.upper_value, Some(110.0.into()));
}

#[tokio::test]
async fn perf_multiple_iterations() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results[0].metrics.len(), 2);
    // Ordered by iteration
    assert_eq!(perf.results[0].metrics[0].iteration.0, 0);
    assert_eq!(perf.results[0].metrics[0].metric.value, 10.0);
    assert_eq!(perf.results[0].metrics[1].iteration.0, 1);
    assert_eq!(perf.results[0].metrics[1].metric.value, 20.0);
}

#[tokio::test]
async fn perf_time_echo() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    // The times passed as query params should be echoed back
    assert!(perf.start_time.is_some());
    assert!(perf.end_time.is_some());
    assert_eq!(perf.start_time.unwrap().timestamp(), ts.timestamp());
    assert_eq!(perf.end_time.unwrap().timestamp(), ts_end.timestamp());
}

// =============================================================================
// Section 11: Testbed / spec
// =============================================================================

#[tokio::test]
async fn perf_spec_from_query_param() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
    assert_eq!(perf.results[0].metrics.len(), 2);
}

// =============================================================================
// Section 12: Project UUID access
// =============================================================================

#[tokio::test]
async fn perf_access_by_project_uuid() {
    let server = TestServer::new().await;
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let perf: JsonPerf = resp.json().await.expect("parse response");
    assert_eq!(perf.results.len(), 1);
}
