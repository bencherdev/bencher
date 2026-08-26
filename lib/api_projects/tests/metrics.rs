#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for project metric endpoints.
//!
//! Metrics are created as part of the report flow, so most tests insert
//! the full data chain directly into the database. The addressed row tests below
//! go through ingest instead, because the point of them is that whatever the
//! report response hands out, the metric endpoint resolves.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, create_empty_parameter, create_test_report, get_project_id},
};
use bencher_json::{
    BenchmarkUuid, BmfVersion, JsonAlerts, JsonOneMetric, JsonReport, MeasureUuid, MetricName,
    MetricUuid, ProjectSlug, ReportBenchmarkUuid,
};
use bencher_schema::{
    model::project::report::{ReportId, upsert_metric_count},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// Create a metric for a given report. Inserts benchmark, measure,
/// `report_benchmark`, and metric rows. Returns the metric UUID.
fn create_test_metric(server: &TestServer, project_id: i32, report_id: i32) -> MetricUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();

    // Benchmark
    let benchmark_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("test-benchmark"),
            schema::benchmark::slug.eq(&format!("test-benchmark-{benchmark_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("Failed to get benchmark ID");
    let parameter_id = create_empty_parameter(&mut conn, benchmark_id);

    // Measure
    let measure_uuid = MeasureUuid::new();
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq("test-measure"),
            schema::measure::slug.eq(&format!("test-measure-{measure_uuid}")),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert measure");
    let measure_id: i32 = schema::measure::table
        .filter(schema::measure::uuid.eq(&measure_uuid))
        .select(schema::measure::id)
        .first(&mut conn)
        .expect("Failed to get measure ID");

    // Report benchmark
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
        .expect("Failed to insert report_benchmark");
    let report_benchmark_id: i32 = schema::report_benchmark::table
        .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
        .select(schema::report_benchmark::id)
        .first(&mut conn)
        .expect("Failed to get report_benchmark ID");

    // Metric
    let metric_uuid = MetricUuid::new();
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(&metric_uuid),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(measure_id),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(42.0),
        ))
        .execute(&mut conn)
        .expect("Failed to insert metric");

    // Keep metric_count_by_report in sync (1 metric inserted)
    let report_id = ReportId::try_from_raw(report_id).expect("valid report ID");
    upsert_metric_count(&mut conn, report_id, 1).expect("Failed to upsert metric_count_by_report");

    metric_uuid
}

/// Attach a job with a spec to a report. Returns the spec name for assertion.
fn attach_job_with_spec(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
) -> String {
    use bencher_json::{JobStatus, JobUuid, Priority, SpecUuid};

    let mut conn = server.db_conn();
    let now = base_timestamp();

    // Spec
    let spec_uuid = SpecUuid::new();
    let spec_name = format!("metric-test-spec-{spec_uuid}");
    let spec_slug = format!("metric-test-spec-{spec_uuid}");
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
        .expect("Failed to insert spec");
    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec_uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to get spec ID");

    // Job
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
        "timeout": 3600
    });

    let job_uuid = JobUuid::new();
    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq("127.0.0.1"),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    spec_name
}

/// Insert `head_version` row to link head -> version for the metric query join chain.
fn link_head_version(server: &TestServer, report_id: i32) {
    let mut conn = server.db_conn();
    let (head_id, version_id): (i32, i32) = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select((schema::report::head_id, schema::report::version_id))
        .first(&mut conn)
        .expect("Failed to get head_id and version_id from report");

    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut conn)
        .expect("Failed to insert head_version");
}

// GET /v0/projects/{project}/metrics/{metric} - not found
#[tokio::test]
async fn metrics_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "metricnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Metric NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Metric NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/metrics/00000000-0000-0000-0000-000000000000"
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

// GET /v0/projects/{project}/metrics/{metric} - basic metric retrieval
#[tokio::test]
async fn metrics_get_basic() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metricbasic@example.com").await;
    let org = server.create_org(&user, "Metric Basic Org").await;
    let project = server
        .create_project(&user, &org, "Metric Basic Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    link_head_version(&server, report_id);
    let metric_uuid = create_test_metric(&server, project_id, report_id);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/metrics/{metric_uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let metric: JsonOneMetric = resp.json().await.expect("Failed to parse response");
    assert_eq!(metric.uuid, metric_uuid);
    assert_eq!(metric.name, MetricName::value());
    assert_eq!(metric.value, 42.0);
    assert_eq!(
        metric
            .metric
            .as_ref()
            .expect("a value row carries the triple")
            .value,
        42.0
    );
    assert_eq!(metric.testbed.name.as_ref(), "test-testbed");
    assert_eq!(metric.benchmark.name.as_ref(), "test-benchmark");
    assert_eq!(metric.measure.name.as_ref(), "test-measure");
}

// GET /v0/projects/{project}/metrics/{metric} - with job spec
#[tokio::test]
async fn metrics_get_with_job_spec() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metricspec@example.com").await;
    let org = server.create_org(&user, "Metric Spec Org").await;
    let project = server
        .create_project(&user, &org, "Metric Spec Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    link_head_version(&server, report_id);
    let metric_uuid = create_test_metric(&server, project_id, report_id);
    let spec_name = attach_job_with_spec(&server, report_id, project.uuid);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/metrics/{metric_uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let metric: JsonOneMetric = resp.json().await.expect("Failed to parse response");
    assert_eq!(metric.uuid, metric_uuid);
    // The testbed should have the spec from the job
    let spec = metric
        .testbed
        .spec
        .expect("Expected testbed.spec to be present");
    assert_eq!(spec.name.as_ref(), spec_name);
}

// GET /v0/projects/{project}/metrics/{metric} - without job, no spec
#[tokio::test]
async fn metrics_get_without_job() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metricnospec@example.com").await;
    let org = server.create_org(&user, "Metric NoSpec Org").await;
    let project = server
        .create_project(&user, &org, "Metric NoSpec Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    link_head_version(&server, report_id);
    let metric_uuid = create_test_metric(&server, project_id, report_id);

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/metrics/{metric_uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let metric: JsonOneMetric = resp.json().await.expect("Failed to parse response");
    assert_eq!(metric.uuid, metric_uuid);
    // No job attached, so testbed.spec should be None
    assert!(
        metric.testbed.spec.is_none(),
        "Expected testbed.spec to be None without a job"
    );
}

// GET /v0/projects/{project}/metrics/{metric} - public project, no auth
#[tokio::test]
async fn metrics_get_public_project_no_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metricpublic@example.com").await;
    let org = server.create_org(&user, "Metric Public Org").await;
    let project = server
        .create_project(&user, &org, "Metric Public Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    link_head_version(&server, report_id);
    let metric_uuid = create_test_metric(&server, project_id, report_id);

    // Projects are public by default -- no auth header
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/metrics/{metric_uuid}"
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let metric: JsonOneMetric = resp.json().await.expect("Failed to parse response");
    assert_eq!(metric.uuid, metric_uuid);
}

// GET /v0/projects/{project}/metrics/{metric} - wrong project returns 404
#[tokio::test]
async fn metrics_get_wrong_project() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "metricwrongproj@example.com")
        .await;
    let org = server.create_org(&user, "Metric WrongProj Org").await;
    let project_a = server.create_project(&user, &org, "Metric Project A").await;
    let project_b = server.create_project(&user, &org, "Metric Project B").await;

    // Create metric in project A
    let project_a_id = get_project_id(&server, project_a.slug.as_ref());
    let report_id = create_test_report(&server, project_a_id);
    link_head_version(&server, report_id);
    let metric_uuid = create_test_metric(&server, project_a_id, report_id);

    // Try to access metric via project B
    let project_b_slug: &str = project_b.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_b_slug}/metrics/{metric_uuid}"
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

// metric_count_by_report upsert correctness
#[tokio::test]
async fn metric_count_by_report_upsert() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metriccount@example.com").await;
    let org = server.create_org(&user, "Metric Count Org").await;
    let project = server
        .create_project(&user, &org, "Metric Count Project")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);

    // create_test_metric upserts metric_count_by_report with count=1
    let _metric_uuid = create_test_metric(&server, project_id, report_id);

    let mut conn = server.db_conn();
    let count: i32 = schema::metric_count_by_report::table
        .filter(schema::metric_count_by_report::report_id.eq(report_id))
        .select(schema::metric_count_by_report::metric_count)
        .first(&mut conn)
        .expect("Failed to query metric_count_by_report");
    assert_eq!(count, 1, "First metric should set count to 1");

    // Simulate a second iteration adding 3 more metrics via upsert
    let report_id = ReportId::try_from_raw(report_id).expect("valid report ID");
    upsert_metric_count(&mut conn, report_id, 3).expect("Failed to upsert metric_count_by_report");

    let count: i32 = schema::metric_count_by_report::table
        .filter(schema::metric_count_by_report::report_id.eq(report_id))
        .select(schema::metric_count_by_report::metric_count)
        .first(&mut conn)
        .expect("Failed to query metric_count_by_report after second upsert");
    assert_eq!(count, 4, "Upsert should accumulate: 1 + 3 = 4");
}

// =============================================================================
// The addressed row: every metric row UUID resolves
// =============================================================================
//
// These go through ingest rather than seeding the database, because the promise is
// about the UUIDs the report response hands out. A bound and a named value are rows
// a report already names; before this shape they were rows the metric endpoint could
// not find.

/// A signed up user with an organization and a project to report into.
struct Fixture {
    project_slug: ProjectSlug,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("metricrow{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Metric Row Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Metric Row Project {label}"))
        .await;
    // This file carries BMF v1 payloads that declare no version at all, and the
    // project gate refuses results that parse as v1 while the project is still at
    // version 0.
    server.set_bmf_version(&project.slug, BmfVersion::V1);
    Fixture {
        project_slug: project.slug,
        token: user.token,
    }
}

/// A threshold model loose enough to compute a boundary from a short history and
/// tight enough that a tenfold jump is an outlier.
fn threshold_models() -> serde_json::Value {
    serde_json::json!({
        "models": {
            "latency": {
                "test": "t_test",
                "min_sample_size": 2,
                "max_sample_size": 64,
                "lower_boundary": 0.98,
                "upper_boundary": 0.98,
            }
        }
    })
}

/// Post one report of `results` and return its raw JSON.
///
/// `day` only has to be distinct and increasing, so reports order the way they were
/// submitted.
async fn report(
    server: &TestServer,
    fixture: &Fixture,
    day: usize,
    results: Vec<String>,
    thresholds: Option<serde_json::Value>,
) -> serde_json::Value {
    let body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": format!("2024-01-{day:02}T00:00:00Z"),
        "end_time": format!("2024-01-{day:02}T00:01:00Z"),
        "results": results,
        "thresholds": thresholds,
    });

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    assert_eq!(status, StatusCode::CREATED, "POST report {day}: {body}");
    serde_json::from_str(&body).expect("Failed to parse the report")
}

/// One BMF v1 payload for a single benchmark's grid points.
fn v1(benchmark: &str, entries: &[serde_json::Value]) -> String {
    serde_json::to_string(&serde_json::json!({ benchmark: entries }))
        .expect("the results serialize")
}

fn entry(parameters: &serde_json::Value, measures: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "parameters": parameters, "measures": measures })
}

/// The UUID of the one row a report named `name` for.
fn row_uuid(report: &serde_json::Value, name: &str) -> MetricUuid {
    let report: JsonReport =
        serde_json::from_value(report.clone()).expect("Failed to parse the report");
    let found = report
        .results
        .as_ref()
        .expect("the report carries its results")
        .iter()
        .flatten()
        .flat_map(|result| &result.measures)
        .flat_map(|measure| &measure.metrics)
        .filter(|metric| metric.name.as_ref() == name)
        .map(|metric| metric.uuid)
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "the report names exactly one {name} row");
    found
        .into_iter()
        .next()
        .expect("the report names a row, just counted")
}

/// GET one metric row and return its status and raw JSON.
async fn get_row(
    server: &TestServer,
    fixture: &Fixture,
    metric_uuid: MetricUuid,
) -> (StatusCode, serde_json::Value) {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/metrics/{metric_uuid}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    if status == StatusCode::OK {
        (
            status,
            serde_json::from_str(&body).expect("Failed to parse the metric"),
        )
    } else {
        (status, serde_json::Value::Null)
    }
}

/// The keys a value row answered with before the addressed row shape, in the order
/// the response carries them. Every one of them keeps its meaning and its value.
///
/// This list and the values asserted against it were read off the previous shape,
/// which answered with exactly these thirteen keys and 404ed on the two bound rows
/// the same report named.
const BEFORE_KEYS: [&str; 13] = [
    "uuid",
    "report",
    "iteration",
    "start_time",
    "end_time",
    "branch",
    "testbed",
    "benchmark",
    "measure",
    "metric",
    "threshold",
    "boundary",
    "alert",
];

/// The keys the addressed row shape adds, and the only difference a value row
/// address sees.
const ADDED_KEYS: [&str; 3] = ["parameter", "name", "value"];

fn keys(metric: &serde_json::Value) -> Vec<String> {
    let mut keys = metric
        .as_object()
        .expect("the metric is an object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn sorted(keys: &[&str]) -> Vec<String> {
    let mut keys = keys.iter().map(|key| (*key).to_owned()).collect::<Vec<_>>();
    keys.sort();
    keys
}

// A value row address answers with everything it answered with before, unchanged,
// plus the three keys the addressed row shape adds. This is the compatibility claim
// stated as a fixture: the key set and the values are both pinned.
#[tokio::test]
async fn metrics_get_value_row_is_unchanged_but_for_the_additions() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "pin").await;

    let posted = report(
        &server,
        &fixture,
        1,
        vec![
            r#"{"bench": {"latency": {"value": 42.0, "lower_value": 40.0, "upper_value": 44.0}}}"#
                .to_owned(),
        ],
        None,
    )
    .await;

    let value_uuid = row_uuid(&posted, "value");
    let (status, metric) = get_row(&server, &fixture, value_uuid).await;
    assert_eq!(status, StatusCode::OK);

    // The key set is the old one plus the additions, and nothing else moved.
    assert_eq!(
        keys(&metric),
        sorted(&[BEFORE_KEYS.as_slice(), ADDED_KEYS.as_slice()].concat()),
    );

    // Every key that was there before, with the value the base shape gave it.
    assert_eq!(metric["uuid"], serde_json::json!(value_uuid));
    assert_eq!(metric["report"], posted["uuid"]);
    assert_eq!(metric["iteration"], serde_json::json!(0));
    assert_eq!(metric["start_time"], posted["start_time"]);
    assert_eq!(metric["end_time"], posted["end_time"]);
    assert_eq!(metric["branch"]["slug"], serde_json::json!("main"));
    assert_eq!(metric["testbed"]["slug"], serde_json::json!("localhost"));
    assert_eq!(metric["benchmark"]["name"], serde_json::json!("bench"));
    assert_eq!(metric["measure"]["slug"], serde_json::json!("latency"));
    assert_eq!(
        metric["metric"],
        serde_json::json!({
            "uuid": value_uuid,
            "value": 42.0,
            "lower_value": 40.0,
            "upper_value": 44.0,
        }),
    );
    assert_eq!(metric["threshold"], serde_json::Value::Null);
    assert_eq!(metric["boundary"], serde_json::Value::Null);
    assert_eq!(metric["alert"], serde_json::Value::Null);

    // The additions: the addressed row's own name and scalar, and the grid point it
    // was measured under.
    assert_eq!(metric["name"], serde_json::json!("value"));
    assert_eq!(metric["value"], serde_json::json!(42.0));
    assert_eq!(metric["parameter"]["set"], serde_json::json!({}));
    assert_eq!(
        metric["parameter"]["benchmark"],
        metric["benchmark"]["uuid"]
    );
}

// A bound is an ordinary row with an ordinary UUID, and the report response hands it
// out. Before this shape it was a 404.
#[tokio::test]
async fn metrics_get_bound_row_resolves() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "bound").await;

    let posted = report(
        &server,
        &fixture,
        1,
        vec![
            r#"{"bench": {"latency": {"value": 42.0, "lower_value": 40.0, "upper_value": 44.0}}}"#
                .to_owned(),
        ],
        None,
    )
    .await;

    let lower_uuid = row_uuid(&posted, "lower_value");
    let (status, metric) = get_row(&server, &fixture, lower_uuid).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(metric["uuid"], serde_json::json!(lower_uuid));
    assert_eq!(metric["name"], serde_json::json!("lower_value"));
    assert_eq!(metric["value"], serde_json::json!(40.0));
    // The triple is absent: it is a convention around a point estimate, and this
    // address does not name one.
    assert_eq!(metric.get("metric"), None);
    // The gate keys stay, as nulls, because nothing gated this row.
    assert_eq!(metric["threshold"], serde_json::Value::Null);
    assert_eq!(metric["boundary"], serde_json::Value::Null);
    assert_eq!(metric["alert"], serde_json::Value::Null);
    // Every context key is the addressed row's own context.
    assert_eq!(metric["report"], posted["uuid"]);
    assert_eq!(metric["benchmark"]["name"], serde_json::json!("bench"));
    assert_eq!(metric["measure"]["slug"], serde_json::json!("latency"));
    assert_eq!(metric["parameter"]["set"], serde_json::json!({}));
}

// A name a report invented resolves the same way the conventional ones do, under the
// grid point it was measured at.
#[tokio::test]
async fn metrics_get_named_row_resolves() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "named").await;

    let posted = report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size_mb": 16 }),
                &serde_json::json!({ "latency": { "value": 42.0, "p99": 97.0 } }),
            )],
        )],
        None,
    )
    .await;

    let p99_uuid = row_uuid(&posted, "p99");
    let (status, metric) = get_row(&server, &fixture, p99_uuid).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(metric["uuid"], serde_json::json!(p99_uuid));
    assert_eq!(metric["name"], serde_json::json!("p99"));
    assert_eq!(metric["value"], serde_json::json!(97.0));
    assert_eq!(metric.get("metric"), None);
    assert_eq!(metric["threshold"], serde_json::Value::Null);
    assert_eq!(metric["boundary"], serde_json::Value::Null);
    assert_eq!(metric["alert"], serde_json::Value::Null);
    assert_eq!(
        metric["parameter"]["set"],
        serde_json::json!({ "size_mb": 16 }),
    );

    // The value row of the same grid point resolves too, and carries its own triple
    // with no bounds, because the report named none.
    let value_uuid = row_uuid(&posted, "value");
    let (status, value_row) = get_row(&server, &fixture, value_uuid).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value_row["metric"],
        serde_json::json!({
            "uuid": value_uuid,
            "value": 42.0,
            "lower_value": null,
            "upper_value": null,
        }),
    );
    assert_eq!(value_row["parameter"]["uuid"], metric["parameter"]["uuid"]);
}

/// The point estimates the fixture reports, run after run. The final report jumps an
/// order of magnitude above the history, which is what the threshold catches.
const HISTORY: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const REGRESSION: f64 = 1_000.0;

// A gated value row answers with the threshold that gated it, the boundary it
// produced, and the alert that boundary raised. The alert is the same alert the
// alerts endpoint lists.
#[tokio::test]
async fn metrics_get_gated_value_row() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "gated").await;

    // Every report also names a `p99`, which lands a second metric row and no
    // boundary. That is what makes the fixture able to see the join: metric
    // identifiers then outrun boundary identifiers, so a gate reached by the wrong
    // column lands on another row instead of coincidentally landing on its own.
    let mut posted = serde_json::Value::Null;
    for (day, value) in HISTORY.into_iter().chain([REGRESSION]).enumerate() {
        posted = report(
            &server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({ "size_mb": 16 }),
                    &serde_json::json!({ "latency": { "value": value, "p99": value * 2.0 } }),
                )],
            )],
            Some(threshold_models()),
        )
        .await;
    }

    let value_uuid = row_uuid(&posted, "value");
    let (status, metric) = get_row(&server, &fixture, value_uuid).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(metric["name"], serde_json::json!("value"));
    assert_eq!(metric["value"], serde_json::json!(REGRESSION));
    assert_ne!(metric["threshold"], serde_json::Value::Null);
    assert_ne!(metric["boundary"], serde_json::Value::Null);

    // The alerts endpoint and the metric endpoint name the same alert.
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/alerts", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let alerts: JsonAlerts = resp.json().await.expect("Failed to parse the alerts");
    assert_eq!(alerts.0.len(), 1, "the regression raises one alert");
    let alert = &alerts.0[0];
    assert_eq!(metric["alert"]["uuid"], serde_json::json!(alert.uuid));
    assert_eq!(metric["boundary"], serde_json::json!(alert.boundary));
    assert_eq!(
        metric["metric"]["uuid"],
        serde_json::json!(alert.metric.uuid),
        "the alert is on the addressed row",
    );

    // The `p99` row of the same grid point was not gated, so it answers with nothing
    // on it.
    let p99_uuid = row_uuid(&posted, "p99");
    let (status, p99) = get_row(&server, &fixture, p99_uuid).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p99["threshold"], serde_json::Value::Null);
    assert_eq!(p99["boundary"], serde_json::Value::Null);
    assert_eq!(p99["alert"], serde_json::Value::Null);
}

// A row of another project is not found, whatever it is named.
#[tokio::test]
async fn metrics_get_named_row_wrong_project() {
    let server = TestServer::new().await;
    let owner = fixture(&server, "foreignowner").await;
    let other = fixture(&server, "foreignother").await;

    let posted = report(
        &server,
        &owner,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size_mb": 16 }),
                &serde_json::json!({ "latency": { "value": 42.0, "p99": 97.0 } }),
            )],
        )],
        None,
    )
    .await;

    for name in ["value", "p99"] {
        let uuid = row_uuid(&posted, name);
        let (status, _) = get_row(&server, &other, uuid).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{name} of another project");
    }
}
