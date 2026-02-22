#![cfg(feature = "plus")]
#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for project metric endpoints.
//!
//! Metrics are created as part of the report flow, so most tests insert
//! the full data chain directly into the database.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, create_test_report, get_project_id},
};
use bencher_json::{BenchmarkUuid, JsonOneMetric, MeasureUuid, MetricUuid, ReportBenchmarkUuid};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// Create a metric for a given report. Inserts benchmark, measure,
/// `report_benchmark`, and metric rows. Returns the metric UUID.
#[expect(clippy::expect_used)]
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
            schema::metric::value.eq(42.0),
        ))
        .execute(&mut conn)
        .expect("Failed to insert metric");

    metric_uuid
}

/// Attach a job with a spec to a report. Returns the spec name for assertion.
#[expect(clippy::expect_used)]
fn attach_job_with_spec(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
) -> String {
    use bencher_json::{JobPriority, JobStatus, JobUuid, SpecUuid};

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
            schema::job::priority.eq(JobPriority::default()),
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
#[expect(clippy::expect_used)]
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
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let metric: JsonOneMetric = resp.json().await.expect("Failed to parse response");
    assert_eq!(metric.uuid, metric_uuid);
    assert_eq!(metric.metric.value, 42.0);
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
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
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
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
