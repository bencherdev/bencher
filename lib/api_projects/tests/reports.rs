#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project report endpoints.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, create_test_report, get_project_id},
};
use bencher_json::{
    BenchmarkUuid, BoundaryUuid, JsonReport, JsonReports, MeasureUuid, MetricUuid, ModelUuid,
    ReportBenchmarkUuid, ThresholdUuid,
};
use bencher_schema::{
    context::DbConnection,
    model::project::report::{ReportId, upsert_metric_count},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// Insert the benchmark, measure, threshold, and model rows needed for
/// report results. Returns (`benchmark_id`, `measure_id`, `threshold_id`, `model_id`).
#[expect(clippy::expect_used, reason = "test helper")]
fn seed_result_infra(
    conn: &mut DbConnection,
    project_id: i32,
    branch_id: i32,
    testbed_id: i32,
) -> (i32, i32, i32, i32) {
    let now = base_timestamp();

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
        .execute(&mut *conn)
        .expect("Failed to insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut *conn)
        .expect("Failed to get benchmark ID");

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
        .execute(&mut *conn)
        .expect("Failed to insert measure");
    let measure_id: i32 = schema::measure::table
        .filter(schema::measure::uuid.eq(&measure_uuid))
        .select(schema::measure::id)
        .first(&mut *conn)
        .expect("Failed to get measure ID");

    let threshold_uuid = ThresholdUuid::new();
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
        .execute(&mut *conn)
        .expect("Failed to insert threshold");
    let threshold_id: i32 = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&threshold_uuid))
        .select(schema::threshold::id)
        .first(&mut *conn)
        .expect("Failed to get threshold ID");

    let model_uuid = ModelUuid::new();
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(&model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(0),
            schema::model::created.eq(&now),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert model");
    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(&model_uuid))
        .select(schema::model::id)
        .first(conn)
        .expect("Failed to get model ID");

    (benchmark_id, measure_id, threshold_id, model_id)
}

/// Seed `count` report results (`report_benchmark` -> metric -> boundary rows)
/// for a report created via `create_test_report`. Uses one benchmark with
/// `count` iterations. Returns the report UUID for the delete URL.
#[expect(clippy::expect_used, reason = "test helper")]
fn seed_report_results(server: &TestServer, project_id: i32, report_id: i32, count: i32) -> String {
    // Batch size per INSERT statement, kept well under SQLite's bind limit.
    const INSERT_BATCH: usize = 1024;

    let mut conn = server.db_conn();

    let (report_uuid, testbed_id, head_id): (String, i32, i32) = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select((
            schema::report::uuid,
            schema::report::testbed_id,
            schema::report::head_id,
        ))
        .first(&mut conn)
        .expect("Failed to get report");
    let branch_id: i32 = schema::head::table
        .filter(schema::head::id.eq(head_id))
        .select(schema::head::branch_id)
        .first(&mut conn)
        .expect("Failed to get branch ID");

    let (benchmark_id, measure_id, threshold_id, model_id) =
        seed_result_infra(&mut conn, project_id, branch_id, testbed_id);

    let report_benchmarks = (0..count)
        .map(|iteration| {
            (
                schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
                schema::report_benchmark::report_id.eq(report_id),
                schema::report_benchmark::iteration.eq(iteration),
                schema::report_benchmark::benchmark_id.eq(benchmark_id),
            )
        })
        .collect::<Vec<_>>();
    for batch in report_benchmarks.chunks(INSERT_BATCH) {
        diesel::insert_into(schema::report_benchmark::table)
            .values(batch.to_vec())
            .execute(&mut conn)
            .expect("Failed to insert report benchmarks");
    }
    let report_benchmark_ids: Vec<i32> = schema::report_benchmark::table
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .select(schema::report_benchmark::id)
        .load(&mut conn)
        .expect("Failed to get report benchmark IDs");

    let metrics = report_benchmark_ids
        .iter()
        .map(|report_benchmark_id| {
            (
                schema::metric::uuid.eq(MetricUuid::new()),
                schema::metric::report_benchmark_id.eq(*report_benchmark_id),
                schema::metric::measure_id.eq(measure_id),
                schema::metric::value.eq(42.0),
            )
        })
        .collect::<Vec<_>>();
    for batch in metrics.chunks(INSERT_BATCH) {
        diesel::insert_into(schema::metric::table)
            .values(batch.to_vec())
            .execute(&mut conn)
            .expect("Failed to insert metrics");
    }
    let metric_ids: Vec<i32> = schema::metric::table
        .filter(schema::metric::report_benchmark_id.eq_any(&report_benchmark_ids))
        .select(schema::metric::id)
        .load(&mut conn)
        .expect("Failed to get metric IDs");

    let boundaries = metric_ids
        .iter()
        .map(|metric_id| {
            (
                schema::boundary::uuid.eq(BoundaryUuid::new()),
                schema::boundary::metric_id.eq(*metric_id),
                schema::boundary::threshold_id.eq(threshold_id),
                schema::boundary::model_id.eq(model_id),
                schema::boundary::baseline.eq(42.0),
                schema::boundary::upper_limit.eq(100.0),
            )
        })
        .collect::<Vec<_>>();
    for batch in boundaries.chunks(INSERT_BATCH) {
        diesel::insert_into(schema::boundary::table)
            .values(batch.to_vec())
            .execute(&mut conn)
            .expect("Failed to insert boundaries");
    }

    let report_id = ReportId::try_from_raw(report_id).expect("valid report ID");
    upsert_metric_count(&mut conn, report_id, count).expect("Failed to upsert metric count");

    report_uuid
}

/// Count the report, `report_benchmark`, metric, and boundary rows for a report.
#[expect(clippy::expect_used, reason = "test helper")]
fn report_row_counts(server: &TestServer, report_id: i32) -> (i64, i64, i64, i64) {
    let mut conn = server.db_conn();
    let reports: i64 = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .count()
        .first(&mut conn)
        .expect("Failed to count reports");
    let report_benchmark_ids: Vec<i32> = schema::report_benchmark::table
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .select(schema::report_benchmark::id)
        .load(&mut conn)
        .expect("Failed to get report benchmark IDs");
    let metric_ids: Vec<i32> = schema::metric::table
        .filter(schema::metric::report_benchmark_id.eq_any(&report_benchmark_ids))
        .select(schema::metric::id)
        .load(&mut conn)
        .expect("Failed to get metric IDs");
    let boundaries: i64 = schema::boundary::table
        .filter(schema::boundary::metric_id.eq_any(&metric_ids))
        .count()
        .first(&mut conn)
        .expect("Failed to count boundaries");
    #[expect(clippy::cast_possible_wrap, reason = "test row counts")]
    (
        reports,
        report_benchmark_ids.len() as i64,
        metric_ids.len() as i64,
        boundaries,
    )
}

/// Delete a report and assert that all of its result rows are gone.
#[expect(clippy::expect_used, reason = "test helper")]
async fn delete_report_and_assert_empty(count: i32) {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportdelresults@example.com")
        .await;
    let org = server.create_org(&user, "Report Del Results Org").await;
    let project = server
        .create_project(&user, &org, "Report Del Results Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let project_id = get_project_id(&server, project_slug);
    let report_id = create_test_report(&server, project_id);
    let report_uuid = seed_report_results(&server, project_id, report_id, count);

    let counts = report_row_counts(&server, report_id);
    assert_eq!(
        counts,
        (1, i64::from(count), i64::from(count), i64::from(count)),
        "seeded report rows"
    );

    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/reports/{}",
            project_slug, report_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "delete report");

    let counts = report_row_counts(&server, report_id);
    assert_eq!(counts, (0, 0, 0, 0), "report rows after delete");
}

// DELETE /v0/projects/{project}/reports/{report} - report with results
// (fewer rows than one delete chunk)
#[tokio::test]
async fn reports_delete_with_results() {
    delete_report_and_assert_empty(5).await;
}

// DELETE /v0/projects/{project}/reports/{report} - report with exactly
// 2 * DELETE_CHUNK_SIZE results, exercising multiple delete chunks and the
// exact-multiple boundary condition
#[tokio::test]
async fn reports_delete_chunked() {
    let count = i32::try_from(2 * api_projects::DELETE_CHUNK_SIZE).expect("count fits in i32");
    delete_report_and_assert_empty(count).await;
}

/// Create a report with two iterations, each with two benchmarks and one measure.
async fn post_report(server: &TestServer, token: &str, project_slug: &str) -> JsonReport {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(bencher_json::AUTHORIZATION, bencher_json::bearer_header(token))
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": [
                "{\"bench_one\": {\"latency\": {\"value\": 100.0}}, \"bench_two\": {\"latency\": {\"value\": 200.0}}}",
                "{\"bench_one\": {\"latency\": {\"value\": 101.0}}, \"bench_two\": {\"latency\": {\"value\": 201.0}}}"
            ]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Failed to parse response")
}

fn assert_counts(counts: &serde_json::Value) {
    assert_eq!(
        counts,
        &serde_json::json!({
            "results": [
                { "benchmarks": 2, "measures": 1 },
                { "benchmarks": 2, "measures": 1 }
            ],
            "alerts": { "total": 0, "active": 0 }
        })
    );
}

// GET /v0/projects/{project}/reports - list reports (empty)
#[tokio::test]
async fn reports_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportlist@example.com").await;
    let org = server.create_org(&user, "Report Org").await;
    let project = server.create_project(&user, &org, "Report Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    // New project should have no reports
    assert!(reports.0.is_empty());
}

// GET /v0/projects/{project}/reports - with pagination
#[tokio::test]
async fn reports_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportpage@example.com").await;
    let org = server.create_org(&user, "Report Page Org").await;
    let project = server
        .create_project(&user, &org, "Report Page Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports?per_page=10&page=1",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/reports/{report} - not found
#[tokio::test]
async fn reports_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Report NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Report NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports/00000000-0000-0000-0000-000000000000",
            project_slug
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

// GET /v0/projects/{project}/reports - results and alerts are collapsed by default
#[tokio::test]
async fn reports_list_collapses_by_default() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportcollapse@example.com")
        .await;
    let org = server.create_org(&user, "Report Collapse Org").await;
    let project = server
        .create_project(&user, &org, "Report Collapse Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    post_report(&server, &user.token, project_slug).await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let reports: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let report = reports
        .as_array()
        .expect("Response is not an array")
        .first()
        .expect("Response is empty");
    // The results and alerts keys are omitted entirely, not just empty
    assert!(report.get("results").is_none());
    assert!(report.get("alerts").is_none());
    assert_counts(report.get("counts").expect("Report missing counts"));

    // The collapsed response still parses as the typed JsonReports
    let reports: JsonReports =
        serde_json::from_value(reports).expect("Failed to parse typed response");
    let report = reports.0.first().expect("Reports are empty");
    assert!(report.results.is_none());
    assert!(report.alerts.is_none());
}

// GET /v0/projects/{project}/reports?expand=true - full results and alerts
#[tokio::test]
async fn reports_list_expand_true_includes_full_report() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportexpand@example.com").await;
    let org = server.create_org(&user, "Report Expand Org").await;
    let project = server
        .create_project(&user, &org, "Report Expand Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    post_report(&server, &user.token, project_slug).await;

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports?expand=true",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    let report = reports.0.first().expect("Reports are empty");
    let results = report.results.as_ref().expect("Report missing results");
    let alerts = report.alerts.as_ref().expect("Report missing alerts");
    // The counts are consistent with the full results and alerts
    assert_eq!(results.len(), report.counts.results.len());
    for (iteration, counts) in results.iter().zip(&report.counts.results) {
        assert_eq!(iteration.len(), counts.benchmarks as usize);
    }
    assert_eq!(alerts.len(), report.counts.alerts.total as usize);
}

// GET /v0/projects/{project}/reports/{report} - always returns the full report
#[tokio::test]
async fn reports_get_one_always_full() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportgetone@example.com").await;
    let org = server.create_org(&user, "Report GetOne Org").await;
    let project = server
        .create_project(&user, &org, "Report GetOne Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let report = post_report(&server, &user.token, project_slug).await;

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/reports/{}",
            project_slug, report.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let report: JsonReport = resp.json().await.expect("Failed to parse response");
    assert!(report.results.is_some());
    assert!(report.alerts.is_some());
}

// POST /v0/projects/{project}/reports - response includes full results, alerts, and counts
#[tokio::test]
async fn reports_post_full_with_counts() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportpost@example.com").await;
    let org = server.create_org(&user, "Report Post Org").await;
    let project = server
        .create_project(&user, &org, "Report Post Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let report = post_report(&server, &user.token, project_slug).await;

    assert!(report.results.is_some());
    assert!(report.alerts.is_some());
    let counts = serde_json::to_value(&report.counts).expect("Failed to serialize counts");
    assert_counts(&counts);
}

// DELETE /v0/projects/{project}/reports/{report} - not found
#[tokio::test]
async fn reports_delete_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportdelnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Report Del NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Report Del NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/reports/00000000-0000-0000-0000-000000000000",
            project_slug
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
