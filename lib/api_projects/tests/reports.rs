#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project report endpoints.

#[cfg(feature = "plus")]
use bencher_api_tests::helpers::{plan_project, post_metrics};
use bencher_api_tests::{
    TestServer,
    helpers::{
        base_timestamp, create_empty_variant, create_test_report, get_empty_variant, get_project_id,
    },
};
use bencher_json::{
    BenchmarkName, BenchmarkUuid, BmfVersion, BoundaryUuid, HeadUuid, JsonProjectKeyCreated,
    JsonReport, JsonReports, MAX_PARAMETER_KEYS, MeasureUuid, MetricName, MetricUuid, ModelUuid,
    ParameterSet, ProjectKeyUuid, ReportBenchmarkUuid, ReportUuid, Slug, ThresholdUuid, UserUuid,
    VersionUuid,
    project::report::{
        Adapter, JsonReportIterationCounts, JsonReportWarning, ParseOutcome, ReportWarningAction,
        ReportWarningResource,
    },
};
#[cfg(feature = "plus")]
use bencher_json::{BranchName, PlanLevel};
use bencher_schema::{
    context::DbConnection,
    model::project::report::{ReportId, upsert_metric_count},
    schema,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
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
    create_empty_variant(conn, benchmark_id);

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
    let variant_id = get_empty_variant(&mut conn, benchmark_id);

    let report_benchmarks = (0..count)
        .map(|iteration| {
            (
                schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
                schema::report_benchmark::report_id.eq(report_id),
                schema::report_benchmark::iteration.eq(iteration),
                schema::report_benchmark::benchmark_id.eq(benchmark_id),
                schema::report_benchmark::variant_id.eq(variant_id),
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
                schema::metric::name.eq(MetricName::value()),
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

/// A report whose version (number 1) is shared by two heads, which also share
/// versions 2 and 3. A third head holds an unrelated version numbered 2.
struct SharedVersions {
    server: TestServer,
    token: String,
    project_slug: String,
    report_uuid: String,
    version_id: i32,
    later_ids: [i32; 2],
    unrelated_id: i32,
    head_ids: [i32; 2],
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn seed_shared_versions() -> SharedVersions {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportdelversions@example.com")
        .await;
    let org = server.create_org(&user, "Report Del Versions Org").await;
    let project = server
        .create_project(&user, &org, "Report Del Versions Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let project_id = get_project_id(&server, project_slug);
    let report_id = create_test_report(&server, project_id);

    let mut conn = server.db_conn();
    let (report_uuid, head_id, version_id): (String, i32, i32) = schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select((
            schema::report::uuid,
            schema::report::head_id,
            schema::report::version_id,
        ))
        .first(&mut conn)
        .expect("Failed to get report");
    let branch_id: i32 = schema::head::table
        .filter(schema::head::id.eq(head_id))
        .select(schema::head::branch_id)
        .first(&mut conn)
        .expect("Failed to get branch ID");

    let later_ids = [
        insert_version(&mut conn, project_id, 2),
        insert_version(&mut conn, project_id, 3),
    ];
    let unrelated_id = insert_version(&mut conn, project_id, 2);
    let other_head_id = insert_head(&mut conn, branch_id);
    let unrelated_head_id = insert_head(&mut conn, branch_id);
    for head in [head_id, other_head_id] {
        for version in [version_id, later_ids[0], later_ids[1]] {
            insert_head_version(&mut conn, head, version);
        }
    }
    insert_head_version(&mut conn, unrelated_head_id, unrelated_id);

    SharedVersions {
        server,
        token: user.token,
        project_slug: project_slug.to_owned(),
        report_uuid,
        version_id,
        later_ids,
        unrelated_id,
        head_ids: [head_id, other_head_id],
    }
}

#[expect(clippy::expect_used, reason = "test helper")]
fn insert_version(conn: &mut DbConnection, project_id: i32, number: i32) -> i32 {
    let version_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(number),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert version");
    schema::version::table
        .filter(schema::version::uuid.eq(&version_uuid))
        .select(schema::version::id)
        .first(conn)
        .expect("Failed to get version ID")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn insert_head(conn: &mut DbConnection, branch_id: i32) -> i32 {
    let head_uuid = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(&head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(&base_timestamp()),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert head");
    schema::head::table
        .filter(schema::head::uuid.eq(&head_uuid))
        .select(schema::head::id)
        .first(conn)
        .expect("Failed to get head ID")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn insert_head_version(conn: &mut DbConnection, head_id: i32, version_id: i32) {
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(conn)
        .expect("Failed to insert head version");
}

#[expect(clippy::expect_used, reason = "test helper")]
fn version_number(conn: &mut DbConnection, version_id: i32) -> Option<i32> {
    schema::version::table
        .filter(schema::version::id.eq(version_id))
        .select(schema::version::number)
        .first(conn)
        .optional()
        .expect("Failed to get version number")
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn delete_shared_version_report(seed: &SharedVersions) -> StatusCode {
    seed.server
        .client
        .delete(seed.server.api_url(&format!(
            "/v0/projects/{}/reports/{}",
            seed.project_slug, seed.report_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&seed.token),
        )
        .send()
        .await
        .expect("Request failed")
        .status()
}

// DELETE /v0/projects/{project}/reports/{report} - removing the last report on a
// version shared by two heads decrements each later version exactly once
#[tokio::test]
async fn reports_delete_renumbers_shared_versions_once() {
    let seed = seed_shared_versions().await;

    assert_eq!(
        delete_shared_version_report(&seed).await,
        StatusCode::NO_CONTENT
    );

    let mut conn = seed.server.db_conn();
    assert_eq!(version_number(&mut conn, seed.version_id), None);
    assert_eq!(version_number(&mut conn, seed.later_ids[0]), Some(1));
    assert_eq!(version_number(&mut conn, seed.later_ids[1]), Some(2));
    assert_eq!(version_number(&mut conn, seed.unrelated_id), Some(2));
    for head_id in seed.head_ids {
        let numbers: Vec<i32> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(head_id))
            .order(schema::version::id)
            .select(schema::version::number)
            .load(&mut conn)
            .expect("Failed to load head versions");
        assert_eq!(numbers, [1, 2], "head {head_id} version numbers");
    }
}

// DELETE /v0/projects/{project}/reports/{report} - a failed version delete
// rolls back the renumber and fails the request
#[tokio::test]
async fn reports_delete_version_failure_rolls_back_renumber() {
    let seed = seed_shared_versions().await;

    let mut conn = seed.server.db_conn();
    diesel::sql_query(format!(
        "CREATE TRIGGER block_version_delete BEFORE DELETE ON version \
         WHEN OLD.id = {} BEGIN SELECT RAISE(ABORT, 'blocked'); END",
        seed.version_id
    ))
    .execute(&mut conn)
    .expect("Failed to create trigger");

    assert_eq!(
        delete_shared_version_report(&seed).await,
        StatusCode::CONFLICT
    );

    assert_eq!(version_number(&mut conn, seed.version_id), Some(1));
    assert_eq!(version_number(&mut conn, seed.later_ids[0]), Some(2));
    assert_eq!(version_number(&mut conn, seed.later_ids[1]), Some(3));
    assert_eq!(version_number(&mut conn, seed.unrelated_id), Some(2));
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
                { "benchmarks": 2, "measures": 1, "lines": 2 },
                { "benchmarks": 2, "measures": 1, "lines": 2 }
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

// POST /v0/projects/{project}/reports - ingest creates each benchmark with its
// empty variant and rides every result on it
#[tokio::test]
async fn reports_ingest_empty_variants() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportvariant@example.com")
        .await;
    let org = server.create_org(&user, "Report Variant Org").await;
    let project = server
        .create_project(&user, &org, "Report Variant Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let report = post_report(&server, &user.token, project_slug).await;
    assert_eq!(
        report.results.as_ref().map(Vec::len),
        Some(2),
        "two iterations were ingested"
    );

    let mut conn = server.db_conn();
    let benchmark_ids: Vec<i32> = schema::benchmark::table
        .select(schema::benchmark::id)
        .load(&mut conn)
        .expect("Failed to load benchmarks");
    assert_eq!(benchmark_ids.len(), 2, "two benchmarks were ingested");

    for benchmark_id in benchmark_ids {
        let parameters: Vec<ParameterSet> = schema::variant::table
            .filter(schema::variant::benchmark_id.eq(benchmark_id))
            .select(schema::variant::parameters)
            .load(&mut conn)
            .expect("Failed to load parameters");
        assert_eq!(
            parameters,
            vec![ParameterSet::default()],
            "benchmark {benchmark_id} must have exactly one empty variant"
        );
    }

    let report_benchmarks: Vec<(i32, i32)> = schema::report_benchmark::table
        .select((
            schema::report_benchmark::benchmark_id,
            schema::report_benchmark::variant_id,
        ))
        .load(&mut conn)
        .expect("Failed to load report benchmarks");
    assert_eq!(
        report_benchmarks.len(),
        4,
        "two iterations of two benchmarks"
    );
    for (benchmark_id, variant_id) in report_benchmarks {
        assert_eq!(
            variant_id,
            get_empty_variant(&mut conn, benchmark_id),
            "every report benchmark rides its own benchmark's empty variant"
        );
    }
}

// POST /v0/projects/{project}/reports - a paid organization past its daily metrics
// limit is refused, and is not told to buy the plan it already has
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_refuses_a_paid_org_past_its_daily_metrics_limit() {
    let server = TestServer::new_with_creation_and_plus_limits(u32::MAX, u32::MAX, 4).await;
    let branch: BranchName = "main".parse().expect("Invalid branch name");
    let (user, project) = plan_project(&server, "paid", Some(PlanLevel::Enterprise)).await;

    let (status, body) = post_metrics(&server, &user, &project, &branch, 4).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = post_metrics(&server, &user, &project, &branch, 1).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("for Metric creation"), "{body}");
    assert!(!body.contains("pricing"), "{body}");
}

// POST /v0/projects/{project}/reports - a paid organization is held to its own daily
// metrics limit, not the claimed one that refuses an organization with no plan
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_admits_a_paid_org_past_the_claimed_limit() {
    let server = TestServer::new_with_creation_and_plus_limits(4, 4, 8).await;
    let branch: BranchName = "main".parse().expect("Invalid branch name");

    let (user, project) = plan_project(&server, "free", None).await;
    let (status, body) = post_metrics(&server, &user, &project, &branch, 4).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = post_metrics(&server, &user, &project, &branch, 1).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("pricing"), "{body}");

    let (user, project) = plan_project(&server, "paid", Some(PlanLevel::Enterprise)).await;
    let (status, body) = post_metrics(&server, &user, &project, &branch, 4).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = post_metrics(&server, &user, &project, &branch, 1).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

// POST /v0/projects/{project}/reports - an organization licensed at the Free level is
// not paid, so it is held to the claimed limit and not the daily metrics limit of a paid
// organization
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_holds_a_free_license_to_the_claimed_limit() {
    let server = TestServer::new_with_creation_and_plus_limits(4, 4, 0).await;
    let branch: BranchName = "main".parse().expect("Invalid branch name");
    let (user, project) = plan_project(&server, "free-license", Some(PlanLevel::Free)).await;

    let (status, body) = post_metrics(&server, &user, &project, &branch, 4).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = post_metrics(&server, &user, &project, &branch, 1).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("pricing"), "{body}");
}

/// Post a report of one BMF v1 benchmark with these measures and return its body.
async fn post_v1_report(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    measures: serde_json::Value,
) -> String {
    let (status, body) = try_post_v1_report(server, token, project_slug, measures).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn try_post_v1_report(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    measures: serde_json::Value,
) -> (StatusCode, String) {
    let results = serde_json::json!({ "bench": [{ "measures": measures }] });
    try_post_ingest(
        server,
        token,
        project_slug,
        &ingest_report(&[results.to_string()], 1),
    )
    .await
}

/// A report that names 34 metrics on one measure, two past the per measure cap of 32.
async fn post_report_past_the_metric_cap(
    server: &TestServer,
    token: &str,
    project_slug: &str,
) -> JsonReport {
    let names = (1..=34)
        .map(|n| (format!("p{n}"), serde_json::json!(n)))
        .collect::<serde_json::Map<_, _>>();
    let body = post_v1_report(
        server,
        token,
        project_slug,
        serde_json::json!({ "latency": names }),
    )
    .await;
    serde_json::from_str(&body).expect("Failed to parse the report")
}

fn report_warning_rows(server: &TestServer, report_id: ReportId) -> i64 {
    schema::report_warning::table
        .filter(schema::report_warning::report_id.eq(report_id))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count report warnings")
}

// POST /v0/projects/{project}/reports - the metric names dropped past the per measure cap are a
// warning on the report
#[tokio::test]
async fn reports_post_warns_of_metric_names_past_the_cap() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportmetricwarn@example.com")
        .await;
    let org = server.create_org(&user, "Report Metric Warn Org").await;
    let project = server
        .create_project(&user, &org, "Report Metric Warn Project")
        .await;

    let report = post_report_past_the_metric_cap(&server, &user.token, project.slug.as_ref()).await;
    assert_eq!(
        report.warnings,
        Some(vec![skipped(ReportWarningResource::Metric, 2)])
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_over_the_license_keeps_its_warnings() {
    use bencher_json::{Entitlements, PlanLevel, project::Visibility};

    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportlicensewarn@example.com")
        .await;
    let org = server.create_org(&user, "Report License Warn Org").await;
    server
        .license_org_with_entitlements(&user, &org, PlanLevel::Enterprise, Entitlements::MIN)
        .await;
    let project = server
        .create_project(&user, &org, "Report License Warn Project")
        .await;
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut server.db_conn())
        .expect("Failed to make the project private");
    let project_slug: &str = project.slug.as_ref();

    // Two measured values pass the license of one, and two names pass the cap of 32.
    let mut latency = (1..=33)
        .map(|n| (format!("p{n}"), serde_json::json!(n)))
        .collect::<serde_json::Map<_, _>>();
    latency.insert("value".to_owned(), serde_json::json!(1.0));
    let measures = serde_json::json!({ "latency": latency, "throughput": { "value": 1.0 } });
    let (status, body) = try_post_v1_report(&server, &user.token, project_slug, measures).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");

    let report_uuid: ReportUuid = schema::report::table
        .select(schema::report::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to load the report");
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/reports/{report_uuid}"
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
    assert_eq!(
        report.warnings,
        Some(vec![skipped(ReportWarningResource::Metric, 2)])
    );
}

// POST /v0/projects/{project}/reports - a report that skipped nothing has no `warnings` key, so
// its bytes are what they were before warnings existed
#[tokio::test]
async fn reports_post_without_warnings_omits_the_key() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportnowarn@example.com").await;
    let org = server.create_org(&user, "Report No Warn Org").await;
    let project = server
        .create_project(&user, &org, "Report No Warn Project")
        .await;

    let body = post_v1_report(
        &server,
        &user.token,
        project.slug.as_ref(),
        serde_json::json!({ "latency": { "value": 1.0 } }),
    )
    .await;
    let report: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse");
    assert!(report.get("warnings").is_none(), "{body}");
}

// GET /v0/projects/{project}/reports - the warnings are only read for one report, so a page of
// reports does not carry them, expanded or not
#[tokio::test]
async fn reports_list_omits_warnings() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportexpandwarn@example.com")
        .await;
    let org = server.create_org(&user, "Report Expand Warn Org").await;
    let project = server
        .create_project(&user, &org, "Report Expand Warn Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let report = post_report_past_the_metric_cap(&server, &user.token, project_slug).await;
    assert_eq!(
        report.warnings,
        Some(vec![skipped(ReportWarningResource::Metric, 2)]),
        "the report has warnings"
    );

    for (query, expand) in [("", false), ("?expand=true", true)] {
        let resp = server
            .client
            .get(server.api_url(&format!("/v0/projects/{project_slug}/reports{query}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK);
        let reports: JsonReports = resp.json().await.expect("Failed to parse response");
        let listed = reports.0.first().expect("Reports are empty");
        assert_eq!(listed.results.is_some(), expand, "expand: {expand}");
        assert!(listed.warnings.is_none(), "expand: {expand}");
    }
}

/// List a project's reports, returning their UUIDs in the order listed and the total count.
async fn list_reports(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    query: &str,
) -> (Vec<ReportUuid>, u64) {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/reports{query}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "{query}");
    let total = resp
        .headers()
        .get("x-total-count")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("No total count");
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    (
        reports.0.into_iter().map(|report| report.uuid).collect(),
        total,
    )
}

async fn post_report_body(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    body: &serde_json::Value,
) -> JsonReport {
    let (status, text) = try_post_ingest(server, token, project_slug, body).await;
    assert_eq!(status, StatusCode::CREATED, "{text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

const BMF_V0: &str = r#"{"bench_one": {"latency": {"value": 100.0}}}"#;
const RUST_BENCH: &str = "test bench_one ... bench:       3,161 ns/iter (+/- 975)\n";

/// A report run with `adapter`, starting `minute` minutes into the day.
fn adapted_report(adapter: &str, results: &str, minute: u32) -> serde_json::Value {
    serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": format!("2024-01-01T00:{minute:02}:00Z"),
        "end_time": format!("2024-01-01T00:{minute:02}:30Z"),
        "results": [results],
        "settings": { "adapter": adapter },
    })
}

// GET /v0/projects/{project}/reports?adapter= - the adapter as the list names it, so a language
// adapter is found under magic, and the total counts only what matches
#[tokio::test]
async fn reports_list_filters_by_adapter_as_listed() {
    let server = TestServer::new().await;
    let (token, slug) = ingest_project(&server, "adapterfilter").await;
    let mut posted = Vec::new();
    for (minute, (adapter, results)) in [
        ("json", BMF_V0),
        ("rust", RUST_BENCH),
        ("magic", BMF_V0),
        ("rust_bench", RUST_BENCH),
    ]
    .into_iter()
    .enumerate()
    {
        let minute = u32::try_from(minute).expect("Too many reports");
        let report = adapted_report(adapter, results, minute);
        posted.push(post_report_body(&server, &token, &slug, &report).await.uuid);
    }
    let [json, rust, magic, rust_bench] = posted.as_slice() else {
        panic!("Expected four reports");
    };

    for (adapter, expected) in [
        ("json", vec![*json]),
        ("magic", vec![*magic, *rust]),
        ("rust", vec![*magic, *rust]),
        ("rust_bench", vec![*rust_bench]),
        ("go_bench", Vec::new()),
    ] {
        let (listed, total) =
            list_reports(&server, &token, &slug, &format!("?adapter={adapter}")).await;
        assert_eq!(listed, expected, "{adapter}");
        assert_eq!(
            total,
            u64::try_from(expected.len()).expect("Too many reports"),
            "{adapter}"
        );
    }
}

// GET /v0/projects/{project}/reports?alerts=active - only the reports with an alert that is still
// active, whatever their other alerts
#[tokio::test]
async fn reports_list_filters_by_alerts_active() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportactivealerts@example.com")
        .await;
    let org = server.create_org(&user, "Report Active Alerts Org").await;
    let project = server
        .create_project(&user, &org, "Report Active Alerts Project")
        .await;
    let slug = project.slug.to_string();

    let report = |day: usize, one: f64, two: f64| {
        let results = serde_json::json!({
            "bench_one": { "latency": { "value": one } },
            "bench_two": { "latency": { "value": two } },
        });
        serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": format!("2024-01-{day:02}T00:00:00Z"),
            "end_time": format!("2024-01-{day:02}T00:01:00Z"),
            "results": [results.to_string()],
        })
    };
    for (day, value) in [10.0, 10.1, 9.9, 10.0, 10.2].into_iter().enumerate() {
        let body = report(day + 1, value, value);
        post_report_body(&server, &user.token, &slug, &body).await;
        if day == 0 {
            let resp = server
                .client
                .post(server.api_url(&format!("/v0/projects/{slug}/thresholds")))
                .header(
                    bencher_json::AUTHORIZATION,
                    bencher_json::bearer_header(&user.token),
                )
                .json(&serde_json::json!({
                    "branch": "main",
                    "testbed": "localhost",
                    "measure": "latency",
                    "test": "t_test",
                    "min_sample_size": 2,
                    "max_sample_size": 64,
                    "upper_boundary": 0.98,
                }))
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), StatusCode::CREATED);
        }
    }
    // Each jump raises one alert; the first is dismissed.
    let dismissed = post_report_body(&server, &user.token, &slug, &report(6, 1000.0, 10.0)).await;
    let active = post_report_body(&server, &user.token, &slug, &report(7, 10.1, 1000.0)).await;
    let alerts = |report: &JsonReport| report.alerts.clone().expect("No alerts");
    assert_eq!(alerts(&dismissed).len(), 1);
    assert_eq!(alerts(&active).len(), 1);
    for alert in alerts(&dismissed) {
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/projects/{slug}/alerts/{}", alert.uuid)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({ "status": "dismissed" }))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let (listed, total) = list_reports(&server, &user.token, &slug, "?alerts=active").await;
    assert_eq!(listed, vec![active.uuid]);
    assert_eq!(total, 1);
}

// GET /v0/projects/{project}/reports - each iteration counts its lines, one per variant, measure,
// and metric name, alike when the report is posted and when the list collapses it
#[tokio::test]
async fn reports_count_the_lines_of_each_iteration() {
    let server = TestServer::new().await;
    let (token, slug) = ingest_project(&server, "reportlines").await;
    let results = serde_json::json!({
        "bench_one": [{ "measures": { "latency": { "value": 1.0, "p99": 2.0 } } }],
        "bench_two": [{ "measures": { "latency": { "value": 3.0 } } }],
    });
    let posted = post_report_body(
        &server,
        &token,
        &slug,
        &ingest_report(&[results.to_string()], 1),
    )
    .await;
    let counts = vec![JsonReportIterationCounts {
        benchmarks: 2,
        measures: 1,
        lines: 3,
    }];
    assert_eq!(posted.counts.results, counts);

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    let listed = reports.0.first().expect("Reports are empty");
    assert_eq!(listed.counts.results, counts);
}

// DELETE /v0/projects/{project}/reports/{report} - deleting a report deletes its warnings
#[tokio::test]
async fn reports_delete_deletes_warnings() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "reportdelwarn@example.com")
        .await;
    let org = server.create_org(&user, "Report Del Warn Org").await;
    let project = server
        .create_project(&user, &org, "Report Del Warn Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let report = post_report_past_the_metric_cap(&server, &user.token, project_slug).await;
    let report_id: ReportId = schema::report::table
        .filter(schema::report::uuid.eq(report.uuid))
        .select(schema::report::id)
        .first(&mut server.db_conn())
        .expect("Failed to load the report id");
    assert_eq!(report_warning_rows(&server, report_id), 1);

    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{project_slug}/reports/{}",
            report.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(report_warning_rows(&server, report_id), 0);
}

/// A signed up user's token and the slug of a new project to report into.
async fn ingest_project(server: &TestServer, label: &str) -> (String, String) {
    let user = server
        .signup("Test User", &format!("ingest{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Ingest Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Ingest Project {label}"))
        .await;
    (user.token, project.slug.to_string())
}

fn ingest_report(results: &[String], bmf_version: u8) -> serde_json::Value {
    serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": results,
        "bmf_version": bmf_version,
        "settings": { "adapter": "json" },
    })
}

async fn try_post_ingest(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    report: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(report)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

async fn post_ingest(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    report: &serde_json::Value,
) -> Option<Vec<JsonReportWarning>> {
    let (status, body) = try_post_ingest(server, token, project_slug, report).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let report: JsonReport = serde_json::from_str(&body).expect("Failed to parse the report");
    report.warnings
}

fn skipped(resource: ReportWarningResource, count: u32) -> JsonReportWarning {
    JsonReportWarning {
        resource,
        action: ReportWarningAction::Skip,
        count,
    }
}

/// Every metric a project ingested, as its benchmark, its variant's parameters, its measure, and
/// its name, sorted.
fn ingested(server: &TestServer, project_slug: &str) -> Vec<(String, String, String, String)> {
    let project_id = get_project_id(server, project_slug);
    let mut ingested = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::benchmark::table)
                .inner_join(schema::variant::table),
        )
        .inner_join(schema::measure::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .select((
            schema::benchmark::name,
            schema::variant::parameters,
            schema::measure::slug,
            schema::metric::name,
        ))
        .load::<(BenchmarkName, ParameterSet, Slug, MetricName)>(&mut server.db_conn())
        .expect("Failed to load the ingested metrics")
        .into_iter()
        .map(|(benchmark, parameters, measure, metric)| {
            (
                benchmark.to_string(),
                parameters.canonical(),
                measure.to_string(),
                metric.to_string(),
            )
        })
        .collect::<Vec<_>>();
    ingested.sort();
    ingested
}

fn row(
    benchmark: &str,
    parameters: &str,
    measure: &str,
    metric: &str,
) -> (String, String, String, String) {
    (
        benchmark.to_owned(),
        parameters.to_owned(),
        measure.to_owned(),
        metric.to_owned(),
    )
}

fn long_benchmark() -> String {
    "b".repeat(BenchmarkName::MAX_LEN + 1)
}

/// One past the longest measure or metric name.
fn long_name() -> String {
    "n".repeat(MetricName::MAX_LEN + 1)
}

// POST /v0/projects/{project}/reports - a variant whose parameters fail validation is skipped
// and warned of, whichever way they fail
#[tokio::test]
async fn reports_post_skips_a_variant_whose_parameters_fail_validation() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "badvariant").await;

    let too_many_keys = (0..=MAX_PARAMETER_KEYS)
        .map(|n| (format!("k{n}"), serde_json::json!(n)))
        .collect::<serde_json::Map<_, _>>();
    let measures = serde_json::json!({ "latency": { "value": 1.0 } });
    let results = serde_json::json!({
        "bench": [
            { "parameters": { "size": 1 }, "measures": measures },
            { "parameters": too_many_keys, "measures": measures },
            { "parameters": { "size": [1] }, "measures": measures },
            { "parameters": { "": 1 }, "measures": measures },
        ]
    });
    let report = ingest_report(&[results.to_string()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![skipped(ReportWarningResource::Variant, 3)])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![row("bench", r#"{"size":1}"#, "latency", "value")]
    );
}

// POST /v0/projects/{project}/reports - an empty string is a parameter value of its own
//
// Kills: an empty string value rejected, which skips its variant with a warning, or stored as if
// the key were unset.
#[tokio::test]
async fn reports_post_ingests_an_empty_parameter_value() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "emptyvalue").await;

    let measures = serde_json::json!({ "latency": { "value": 1.0 } });
    let results = serde_json::json!({
        "bench": [
            { "parameters": { "env": "" }, "measures": measures },
            { "measures": measures },
        ]
    });
    let report = ingest_report(&[results.to_string()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(warnings, None);
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("bench", r#"{"env":""}"#, "latency", "value"),
            row("bench", "{}", "latency", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - a metric whose name fails validation is skipped and warned
// of, and the rest of its measure is ingested
#[tokio::test]
async fn reports_post_skips_a_metric_whose_name_fails_validation() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "badmetric").await;

    let results = serde_json::json!({
        "bench": [{ "measures": { "latency": { "value": 1.0, long_name(): 2.0 } } }]
    });
    let report = ingest_report(&[results.to_string()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![skipped(ReportWarningResource::Metric, 1)])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![row("bench", "{}", "latency", "value")]
    );
}

/// One BMF v1 payload with one bad benchmark, variant, measure, and metric among good ones. The
/// bad variant also names a bad measure and a bad metric, which are not counted again.
fn one_of_each() -> String {
    serde_json::json!({
        "good": [
            {
                "parameters": { "size": 1 },
                "measures": {
                    "latency": { "value": 1.0, "p99": 2.0, long_name(): 3.0 },
                    long_name(): { "value": 4.0 },
                },
            },
            {
                "parameters": { "size": [2] },
                "measures": {
                    "latency": { "value": 5.0, long_name(): 6.0 },
                    long_name(): { "value": 7.0 },
                },
            },
        ],
        long_benchmark(): [{ "measures": { "latency": { "value": 8.0 } } }],
    })
    .to_string()
}

// POST /v0/projects/{project}/reports - one payload skips each item that fails at its own level,
// with one warning for each, and ingests every good item
#[tokio::test]
async fn reports_post_skips_one_of_each_in_one_v1_payload() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "oneofeach").await;

    let report = ingest_report(&[one_of_each()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![
            skipped(ReportWarningResource::Benchmark, 1),
            skipped(ReportWarningResource::Variant, 1),
            skipped(ReportWarningResource::Measure, 1),
            skipped(ReportWarningResource::Metric, 1),
        ])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("good", r#"{"size":1}"#, "latency", "p99"),
            row("good", r#"{"size":1}"#, "latency", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - a skip is counted in every payload it occurs in
#[tokio::test]
async fn reports_post_counts_a_skip_in_each_payload() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "eachpayload").await;

    let report = ingest_report(&[one_of_each(), one_of_each()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![
            skipped(ReportWarningResource::Benchmark, 2),
            skipped(ReportWarningResource::Variant, 2),
            skipped(ReportWarningResource::Measure, 2),
            skipped(ReportWarningResource::Metric, 2),
        ])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("good", r#"{"size":1}"#, "latency", "p99"),
            row("good", r#"{"size":1}"#, "latency", "p99"),
            row("good", r#"{"size":1}"#, "latency", "value"),
            row("good", r#"{"size":1}"#, "latency", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - a report level field stays strict: a bad branch name is a
// 400 and creates no report, however lenient its results are
#[tokio::test]
async fn reports_post_with_a_bad_branch_name_is_a_bad_request() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "badbranch").await;

    let mut report = ingest_report(&[one_of_each()], 1);
    report["branch"] = serde_json::json!("");
    let (status, body) = try_post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let project_id = get_project_id(&server, &project_slug);
    let reports: i64 = schema::report::table
        .filter(schema::report::project_id.eq(project_id))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count the reports");
    assert_eq!(reports, 0);
}

// POST /v0/projects/{project}/reports - a benchmark named by a UUID the project does not have is
// a failed lookup, not a skip
#[tokio::test]
async fn reports_post_with_an_unknown_benchmark_uuid_fails() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "unknownuuid").await;

    let results = serde_json::json!({
        BenchmarkUuid::new().to_string(): { "latency": { "value": 1.0 } },
    });
    let report = ingest_report(&[results.to_string()], 0);
    let (status, body) = try_post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

// POST /v0/projects/{project}/reports - a new benchmark the daily creation ceiling refuses is
// skipped and warned of in every payload it occurs in, and the benchmark it admitted is ingested
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_skips_a_benchmark_past_the_creation_ceiling() {
    // One creation of each resource: the report's branch, testbed, measure, and first benchmark.
    let server = TestServer::new_with_creation_limits(1, 1).await;
    let (token, project_slug) = ingest_project(&server, "benchmarkceiling").await;

    let first = serde_json::json!({ "first": { "latency": { "value": 1.0 } } });
    let both = serde_json::json!({
        "first": { "latency": { "value": 2.0 } },
        "second": { "latency": { "value": 3.0 } },
    });
    let report = ingest_report(&[first.to_string(), both.to_string(), both.to_string()], 0);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![skipped(ReportWarningResource::Benchmark, 2)])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("first", "{}", "latency", "value"),
            row("first", "{}", "latency", "value"),
            row("first", "{}", "latency", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - a new measure the daily creation ceiling refuses is skipped
// and warned of, and the measure it admitted is ingested
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_skips_a_measure_past_the_creation_ceiling() {
    // One creation of each resource: the report's branch, testbed, benchmark, and first measure.
    let server = TestServer::new_with_creation_limits(1, 1).await;
    let (token, project_slug) = ingest_project(&server, "measureceiling").await;

    let first = serde_json::json!({ "bench": { "latency": { "value": 1.0 } } });
    let both = serde_json::json!({
        "bench": {
            "latency": { "value": 2.0 },
            "throughput": { "value": 3.0 },
        }
    });
    let report = ingest_report(&[first.to_string(), both.to_string()], 0);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![skipped(ReportWarningResource::Measure, 1)])
    );
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("bench", "{}", "latency", "value"),
            row("bench", "{}", "latency", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - a BMF v1 variant whose every measure the daily creation
// ceiling refuses measured nothing, so it writes no row
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_writes_no_row_for_a_variant_whose_every_measure_is_refused() {
    // Two creations of each resource: the first payload's two measures fill the measure budget.
    let server = TestServer::new_with_creation_limits(2, 2).await;
    let (token, project_slug) = ingest_project(&server, "emptyvariant").await;

    let first = serde_json::json!({
        "bench": [{ "measures": { "latency": { "value": 1.0 }, "throughput": { "value": 2.0 } } }]
    });
    let refused = serde_json::json!({
        "bench": [{ "parameters": { "size": 1 }, "measures": { "other": { "value": 3.0 } } }]
    });
    let report = ingest_report(&[first.to_string(), refused.to_string()], 1);
    let warnings = post_ingest(&server, &token, &project_slug, &report).await;

    assert_eq!(
        warnings,
        Some(vec![skipped(ReportWarningResource::Measure, 1)])
    );
    let project_id = get_project_id(&server, &project_slug);
    let report_benchmarks: i64 = schema::report_benchmark::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count the report benchmarks");
    assert_eq!(report_benchmarks, 1);
}

// POST /v0/projects/{project}/reports - a threshold declared on a new measure that the daily
// creation ceiling refuses is skipped and warned of, in either form of `thresholds.models`, and the
// rest of the report is ingested
#[cfg(feature = "plus")]
#[tokio::test]
async fn reports_post_skips_a_threshold_on_a_measure_past_the_creation_ceiling() {
    // One creation of each resource per project, so of two new measures only the first created
    // fits, and the other is refused for its threshold and again for its results.
    let server = TestServer::new_with_creation_limits(1, 1).await;

    let model = serde_json::json!({ "test": "percentage", "upper_boundary": 0.1 });
    let measures =
        serde_json::json!({ "latency": { "value": 1.0 }, "throughput": { "value": 2.0 } });
    let map = (
        serde_json::json!({ "bench": measures }),
        0,
        serde_json::json!({ "latency": model, "throughput": model }),
    );
    let list = (
        serde_json::json!({ "bench": [{ "measures": measures }] }),
        1,
        serde_json::json!([
            { "measure": "latency", "metric": "value", "model": model },
            { "measure": "throughput", "metric": "value", "model": model },
        ]),
    );
    for (label, (results, bmf_version, models)) in [("map", map), ("list", list)] {
        let (token, project_slug) = ingest_project(&server, &format!("threshold{label}")).await;
        let mut report = ingest_report(&[results.to_string()], bmf_version);
        report["thresholds"] = serde_json::json!({ "models": models });
        let warnings = post_ingest(&server, &token, &project_slug, &report).await;

        assert_eq!(
            warnings,
            Some(vec![
                skipped(ReportWarningResource::Measure, 1),
                skipped(ReportWarningResource::Threshold, 1),
            ]),
            "{label}"
        );
        assert_eq!(ingested(&server, &project_slug).len(), 1, "{label}");
        let project_id = get_project_id(&server, &project_slug);
        let thresholds: i64 = schema::threshold::table
            .filter(schema::threshold::project_id.eq(project_id))
            .count()
            .get_result(&mut server.db_conn())
            .expect("Failed to count the thresholds");
        assert_eq!(thresholds, 1, "{label}");
    }
}

// A report names the project key that created it in the response, the list, and the report
// itself, and still names it once the key is revoked. A report a user created names no key.
#[tokio::test]
async fn reports_name_the_project_key_that_created_them() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportkey@example.com").await;
    let org = server.create_org(&user, "Report Key Org").await;
    let project = server
        .create_project(&user, &org, "Report Key Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let key = create_project_key(&server, &user.token, project_slug, "ci-key").await;
    let named_key = Some((key.uuid, "ci-key".to_owned()));

    let by_key = post_report(&server, key.key.as_ref(), project_slug).await;
    let by_user = post_report(&server, &user.token, project_slug).await;
    assert_eq!(creator(&by_key), (None, named_key.clone()));
    assert_eq!(creator(&by_user), (Some(user.uuid), None));

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse response");
    let mut listed: Vec<_> = reports
        .0
        .iter()
        .map(|report| (report.uuid, creator(report)))
        .collect();
    listed.sort_by_key(|(uuid, _)| *uuid);
    let mut expected = vec![
        (by_key.uuid, (None, named_key.clone())),
        (by_user.uuid, (Some(user.uuid), None)),
    ];
    expected.sort_by_key(|(uuid, _)| *uuid);
    assert_eq!(listed, expected);

    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{project_slug}/keys/{}", key.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/reports/{}",
            by_key.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let revoked: JsonReport = resp.json().await.expect("Failed to parse response");
    assert_eq!(creator(&revoked), (None, named_key));
}

async fn create_project_key(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    name: &str,
) -> JsonProjectKeyCreated {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/keys")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Failed to parse response")
}

/// Who created a report: the user's UUID, and the project key's UUID and name.
fn creator(report: &JsonReport) -> (Option<UserUuid>, Option<(ProjectKeyUuid, String)>) {
    (
        report.user.as_ref().map(|user| user.uuid),
        report
            .project_key
            .as_ref()
            .map(|key| (key.uuid, key.name.as_ref().to_owned())),
    )
}

const BMF_V1: &str = r#"{"bench_one": [{"measures": {"latency": {"median": 100.0}}}]}"#;

/// A report of `results` declared at `bmf_version`, or at the project's when `None`.
fn parse_report(adapter: &str, results: &[&str], bmf_version: Option<u8>) -> serde_json::Value {
    let mut report = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": results,
        "settings": { "adapter": adapter },
    });
    if let Some(bmf_version) = bmf_version
        && let Some(object) = report.as_object_mut()
    {
        object.insert("bmf_version".to_owned(), bmf_version.into());
    }
    report
}

type Parse = (Option<BmfVersion>, Option<Adapter>, Option<ParseOutcome>);

fn parse_of(report: &JsonReport) -> (Option<BmfVersion>, Option<ParseOutcome>) {
    (report.bmf_version, report.parse_outcome)
}

/// The parse the newest report of the project recorded.
fn stored_parse(server: &TestServer, project_slug: &str) -> Parse {
    let project_id = get_project_id(server, project_slug);
    schema::report::table
        .filter(schema::report::project_id.eq(project_id))
        .order(schema::report::id.desc())
        .select((
            schema::report::bmf_version,
            schema::report::resolved_adapter,
            schema::report::parse_outcome,
        ))
        .first(&mut server.db_conn())
        .expect("Failed to read the report")
}

// POST /v0/projects/{project}/reports - a report records the BMF version it was read at, the leaf a
// version 1 report resolved to, and how its results parsed; its response and its read carry the
// version and the outcome
//
// Kills: a version taken from anything but the declared one, else the project's; a version 0
// report that records a leaf; a version 1 report through `magic`, `json`, or a harness node that
// records none or the node; an outcome that calls results with no metric parsed; and a response
// that leaves out the stored version or outcome.
#[tokio::test]
async fn reports_post_records_the_parse() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "parse").await;
    let parsed_v0 = (Some(BmfVersion::V0), None, Some(ParseOutcome::Parsed));
    let parsed_v1 = (
        Some(BmfVersion::V1),
        Some(Adapter::JsonV1),
        Some(ParseOutcome::Parsed),
    );
    let empty_v0 = (Some(BmfVersion::V0), None, Some(ParseOutcome::Empty));
    let empty_v1 = (
        Some(BmfVersion::V1),
        Some(Adapter::JsonV1),
        Some(ParseOutcome::Empty),
    );
    for (adapter, results, bmf_version, expected) in [
        ("magic", vec![BMF_V0], None, parsed_v0),
        ("rust_bench", vec![RUST_BENCH], Some(0), parsed_v0),
        (
            "rust_bench",
            vec![RUST_BENCH],
            Some(1),
            (
                Some(BmfVersion::V1),
                Some(Adapter::RustBenchText),
                Some(ParseOutcome::Parsed),
            ),
        ),
        ("magic", vec![RUST_BENCH, BMF_V0], Some(0), parsed_v0),
        ("magic", vec![BMF_V1], Some(1), parsed_v1),
        ("json", vec![BMF_V1, BMF_V1], Some(1), parsed_v1),
        ("json_v1", vec![BMF_V1], Some(1), parsed_v1),
        ("magic", vec![], None, empty_v0),
        ("json", vec!["{}"], Some(0), empty_v0),
        ("json", vec![r#"{"bench_one": {}}"#], Some(0), empty_v0),
        ("magic", vec!["{}"], Some(1), empty_v1),
        (
            "json",
            vec![r#"{"bench_one": [{"measures": {}}]}"#],
            Some(1),
            empty_v1,
        ),
        (
            "json",
            vec![r#"{"bench_one": [{"measures": {"latency": {}}}]}"#],
            Some(1),
            empty_v1,
        ),
    ] {
        let case = format!("{adapter} {results:?} at {bmf_version:?}");
        let report = post_report_body(
            &server,
            &token,
            &project_slug,
            &parse_report(adapter, &results, bmf_version),
        )
        .await;
        let (version, _, outcome) = expected;
        assert_eq!(parse_of(&report), (version, outcome), "{case}");
        assert_eq!(stored_parse(&server, &project_slug), expected, "{case}");
        let read = get_report(&server, &token, &project_slug, report.uuid).await;
        assert_eq!(parse_of(&read), (version, outcome), "{case}");
    }
}

// POST /v0/projects/{project}/reports - results that fail to parse are refused as they always
// were, and the report they leave behind records the failure
//
// Kills: a failed parse that records nothing, records the outcome of the iterations that did
// parse, or records a leaf.
#[tokio::test]
async fn reports_post_records_a_failed_parse() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "parsefail").await;
    let failed_v0 = (Some(BmfVersion::V0), None, Some(ParseOutcome::Failed));
    let failed_v1 = (Some(BmfVersion::V1), None, Some(ParseOutcome::Failed));
    for (adapter, results, bmf_version, expected) in [
        ("magic", vec!["not benchmark output"], Some(0), failed_v0),
        (
            "magic",
            vec![BMF_V0, "not benchmark output"],
            Some(0),
            failed_v0,
        ),
        (
            "magic",
            vec![BMF_V1, "not benchmark output"],
            Some(1),
            failed_v1,
        ),
        ("rust_bench", vec![BMF_V0], None, failed_v0),
        ("magic", vec![BMF_V0], Some(1), failed_v1),
    ] {
        let case = format!("{adapter} {results:?} at {bmf_version:?}");
        let (status, body) = try_post_ingest(
            &server,
            &token,
            &project_slug,
            &parse_report(adapter, &results, bmf_version),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{case}: {body}");
        assert_eq!(stored_parse(&server, &project_slug), expected, "{case}");
    }
}

async fn get_report(
    server: &TestServer,
    token: &str,
    project_slug: &str,
    report_uuid: ReportUuid,
) -> JsonReport {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/reports/{report_uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("Failed to parse the report")
}

// POST /v0/projects/{project}/reports - only the listed measures are stored, each matched by name
// or slug, at both versions: a dropped measure leaves no measure, metric, or series, a benchmark
// left with nothing leaves no benchmark or report benchmark, and a listed measure the results
// lack is ignored
//
// Kills: a filter that keeps an unlisted measure, matches only the spelling the results use or
// misses a built-in measure listed by its name, creates a dropped measure or the benchmark it
// emptied, writes their report benchmark or series, refuses or creates a listed measure the
// results lack, or records a parse the filter emptied.
#[tokio::test]
async fn reports_post_keeps_only_the_listed_measures() {
    let server = TestServer::new().await;
    for bmf_version in [0, 1] {
        let (token, project_slug) = ingest_project(&server, &format!("keep{bmf_version}")).await;
        let (measures_before, _, _, _) = stored(&server, &project_slug);
        let report = keeping(
            &kept_and_dropped(bmf_version),
            bmf_version,
            &["Latency", "throughput", "My Kept", "Ge", "never-reported"],
        );
        let (status, body) = try_post_ingest(&server, &token, &project_slug, &report).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let report: JsonReport = serde_json::from_str(&body).expect("Failed to parse the report");
        assert_eq!(report.warnings, None, "{bmf_version}");
        assert_eq!(
            report.parse_outcome,
            Some(ParseOutcome::Parsed),
            "{bmf_version}"
        );

        assert_eq!(
            ingested(&server, &project_slug),
            vec![
                row("bench_kept", "{}", "global-bus-events", "value"),
                row("bench_kept", "{}", "latency", "value"),
                row("bench_kept", "{}", "my-kept", "value"),
                row("bench_kept", "{}", "throughput", "value"),
            ],
            "{bmf_version}"
        );
        let mut measures = measures_before;
        measures
            .extend(["global-bus-events", "latency", "my-kept", "throughput"].map(str::to_owned));
        measures.sort();
        measures.dedup();
        let series = if cfg!(feature = "plus") { 4 } else { 0 };
        assert_eq!(
            stored(&server, &project_slug),
            (measures, vec!["bench_kept".to_owned()], 1, series),
            "{bmf_version}"
        );
    }
}

// POST /v0/projects/{project}/reports - a list of measures none of which the results hold drops
// every measure, and the report still parsed; an empty list keeps every measure
//
// Kills: a filter that keeps everything when nothing it lists is present, an empty list that
// drops everything, and a report the filter emptied recorded as empty or failed.
#[tokio::test]
async fn reports_post_keeps_nothing_unlisted_and_everything_for_an_empty_list() {
    let server = TestServer::new().await;
    for bmf_version in [0, 1] {
        let (token, project_slug) = ingest_project(&server, &format!("none{bmf_version}")).await;
        let report = keeping(
            &kept_and_dropped(bmf_version),
            bmf_version,
            &["never-reported"],
        );
        let report = post_report_body(&server, &token, &project_slug, &report).await;
        assert_eq!(
            report.parse_outcome,
            Some(ParseOutcome::Parsed),
            "{bmf_version}"
        );
        assert_eq!(
            ingested(&server, &project_slug),
            Vec::new(),
            "{bmf_version}"
        );
        let (_, benchmarks, report_benchmarks, _) = stored(&server, &project_slug);
        assert_eq!(
            (benchmarks, report_benchmarks),
            (Vec::new(), 0),
            "{bmf_version}"
        );

        let (token, project_slug) = ingest_project(&server, &format!("all{bmf_version}")).await;
        let report = keeping(&kept_and_dropped(bmf_version), bmf_version, &[]);
        post_report_body(&server, &token, &project_slug, &report).await;
        assert_eq!(ingested(&server, &project_slug).len(), 6, "{bmf_version}");
    }
}

// POST /v0/projects/{project}/reports - a list of 65 measures keeps its first 64, so a measure
// listed 65th is dropped, and the report warns of the one entry it cut, at both versions
//
// Kills: a list that is refused or not cut, a cut that keeps the last 64 or keeps 65, and a cut
// that warns of nothing or under any other resource or action.
#[tokio::test]
async fn reports_post_cuts_a_measure_listed_65th() {
    let server = TestServer::new().await;
    for bmf_version in [0, 1] {
        let (token, project_slug) = ingest_project(&server, &format!("cut{bmf_version}")).await;
        let listed = listed_with_latency_last(65);
        let listed = listed.iter().map(String::as_str).collect::<Vec<_>>();
        let report = keeping(&kept_and_dropped(bmf_version), bmf_version, &listed);
        let report = post_report_body(&server, &token, &project_slug, &report).await;
        assert_eq!(
            report.warnings,
            Some(vec![JsonReportWarning {
                resource: ReportWarningResource::Measure,
                action: ReportWarningAction::Ignore,
                count: 1,
            }]),
            "{bmf_version}"
        );
        assert_eq!(
            ingested(&server, &project_slug),
            vec![row("bench_kept", "{}", "throughput", "value")],
            "{bmf_version}"
        );
    }
}

// POST /v0/projects/{project}/reports - a list of exactly 64 measures keeps all of them, with no
// warning
//
// Kills: a cut that keeps 63, and a warning when nothing is cut.
#[tokio::test]
async fn reports_post_keeps_all_64_listed_measures() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "keep64").await;
    let listed = listed_with_latency_last(64);
    let listed = listed.iter().map(String::as_str).collect::<Vec<_>>();
    let report = keeping(&kept_and_dropped(1), 1, &listed);
    let report = post_report_body(&server, &token, &project_slug, &report).await;
    assert_eq!(report.warnings, None);
    assert_eq!(
        ingested(&server, &project_slug),
        vec![
            row("bench_kept", "{}", "latency", "value"),
            row("bench_kept", "{}", "throughput", "value"),
        ]
    );
}

// POST /v0/projects/{project}/reports - the average setting is ignored with a warning at BMF
// version 1, and works as it always has at version 0
//
// Kills: an average at version 1 that warns of nothing, warns more than once, or warns under any
// other resource or action, and an average at version 0 that warns.
#[tokio::test]
async fn reports_post_ignores_the_average_at_version_1() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "average").await;
    for (bmf_version, results, warnings) in [
        (0, BMF_V0, None),
        (
            1,
            BMF_V1,
            Some(vec![JsonReportWarning {
                resource: ReportWarningResource::Average,
                action: ReportWarningAction::Ignore,
                count: 1,
            }]),
        ),
    ] {
        let report = settled_report(
            results,
            bmf_version,
            &serde_json::json!({ "adapter": "json", "average": "median" }),
        );
        let report = post_report_body(&server, &token, &project_slug, &report).await;
        assert_eq!(report.warnings, warnings, "{bmf_version}");
        let read = get_report(&server, &token, &project_slug, report.uuid).await;
        assert_eq!(read.warnings, warnings, "{bmf_version}");
    }
}

// POST /v0/projects/{project}/reports - a measure whose name makes no slug is kept when it is
// listed by that name
//
// Kills: a filter that cannot match a measure whose name makes no slug.
#[tokio::test]
async fn reports_post_keeps_a_listed_measure_whose_name_makes_no_slug() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "unslugged").await;
    let results = serde_json::json!({
        "bench": [{ "measures": { "~~~": { "value": 1.0 }, "latency": { "value": 2.0 } } }],
    });
    let report = keeping(&results, 1, &["~~~"]);
    post_report_body(&server, &token, &project_slug, &report).await;
    let ingested = ingested(&server, &project_slug);
    assert_eq!(ingested.len(), 1);
    assert!(
        ingested
            .iter()
            .all(|(_, _, measure, _)| measure != "latency"),
        "{ingested:?}"
    );
}

// POST /v0/projects/{project}/reports - a measure the project holds is kept when it is listed by
// its name, though the results name it by a slug that the name does not make
//
// Kills: a filter that resolves a listed name to the measure it would create instead of the
// measure the project holds.
#[tokio::test]
async fn reports_post_keeps_an_existing_measure_listed_by_a_name_whose_slug_differs() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "existing").await;
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/measures")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({ "name": "Wall Clock", "slug": "wall", "units": "seconds" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let results = serde_json::json!({
        "bench": [{ "measures": { "wall": { "value": 1.0 }, "latency": { "value": 2.0 } } }],
    });
    let report = keeping(&results, 1, &["Wall Clock"]);
    post_report_body(&server, &token, &project_slug, &report).await;
    assert_eq!(
        ingested(&server, &project_slug),
        vec![row("bench", "{}", "wall", "value")]
    );
}

// POST /v0/projects/{project}/reports - a measure a version 1 leaf names at run time is created in
// the unit its harness gives, and a measure the project already holds keeps its own unit
//
// Kills: a measure named at run time created with the generic unit, and a leaf's unit applied to
// a measure the project already holds in a unit of its own.
#[tokio::test]
async fn reports_post_creates_a_measure_named_at_run_time_in_its_unit() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "units").await;
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/measures")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({ "name": "events", "slug": "events", "units": "branches" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    for results in [CRITERION_CYCLES_PER_BYTE, CRITERION_PERF_EVENTS] {
        post_report_body(
            &server,
            &token,
            &project_slug,
            &parse_report("rust_criterion", &[results], Some(1)),
        )
        .await;
    }
    assert_eq!(
        measure_units(&server, &project_slug),
        [("cpb", "cpb"), ("cycles", "cycles"), ("events", "branches")]
            .map(|(slug, units)| (slug.to_owned(), units.to_owned()))
    );
}

// POST /v0/projects/{project}/reports - a measure a threshold created in the generic unit takes the
// unit a version 1 leaf gives it once the results parse
//
// Kills: a leaf's unit that never replaces the generic unit of a measure the project holds.
#[tokio::test]
async fn reports_post_replaces_the_generic_unit_of_a_measure_a_threshold_created() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "placeholder").await;
    let mut report = parse_report("rust_criterion", &[CRITERION_CYCLES_PER_BYTE], Some(1));
    report["thresholds"] = serde_json::json!({
        "models": [{
            "measure": "cpb",
            "metric": "typical",
            "model": { "test": "percentage", "upper_boundary": 0.25 },
        }]
    });
    post_report_body(&server, &token, &project_slug, &report).await;
    assert_eq!(
        measure_units(&server, &project_slug),
        [("cpb", "cpb"), ("cycles", "cycles")]
            .map(|(slug, units)| (slug.to_owned(), units.to_owned()))
    );
}

// POST /v0/projects/{project}/reports - a leaf that names a built-in measure in a unit of its own
// creates the built-in in the built-in's unit
//
// Kills: a leaf's unit applied to a built-in measure.
#[tokio::test]
async fn reports_post_creates_a_built_in_measure_in_its_own_unit() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "builtin").await;
    let results = "fib 15                  time:   [1.0 file-size 2.0 file-size 3.0 file-size]\n";
    post_report_body(
        &server,
        &token,
        &project_slug,
        &parse_report("rust_criterion", &[results], Some(1)),
    )
    .await;
    let units = measure_units(&server, &project_slug);
    assert!(
        units.contains(&("file-size".to_owned(), "bytes (B)".to_owned())),
        "{units:?}"
    );
}

/// The slug and units of each measure a project holds that a version 1 Rust leaf names at run
/// time or a criterion plugin reports, sorted by slug.
fn measure_units(server: &TestServer, project_slug: &str) -> Vec<(String, String)> {
    let project_id = get_project_id(server, project_slug);
    schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq_any(["cpb", "cycles", "events", "file-size"]))
        .order(schema::measure::slug)
        .select((schema::measure::slug, schema::measure::units))
        .load::<(String, String)>(&mut server.db_conn())
        .expect("Failed to load the measures")
}

// POST /v0/projects/{project}/reports - the C++ and Go leaves create CPU time, allocations, and
// allocated bytes in their own units
//
// Kills: a new built-in measure missing from the server's built-ins, which creates it in the
// generic unit, and allocated bytes created without the unit the Go leaf gives it.
#[tokio::test]
async fn reports_post_creates_the_cpp_and_go_measures_in_their_units() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "cppgo").await;
    for (adapter, results) in [("cpp_google", GOOGLE_MEMORY), ("go_bench", GO_BENCHMEM)] {
        post_report_body(
            &server,
            &token,
            &project_slug,
            &parse_report(adapter, &[results], Some(1)),
        )
        .await;
    }
    let project_id = get_project_id(&server, &project_slug);
    let measures = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq_any(["allocated", "allocations", "cpu-time"]))
        .order(schema::measure::slug)
        .select((
            schema::measure::slug,
            schema::measure::name,
            schema::measure::units,
        ))
        .load::<(String, String, String)>(&mut server.db_conn())
        .expect("Failed to load the measures");
    assert_eq!(
        measures,
        [
            ("allocated", "allocated", "bytes (B)"),
            ("allocations", "Allocations", "allocations"),
            ("cpu-time", "CPU Time", "nanoseconds (ns)"),
        ]
        .map(|(slug, name, units)| (slug.to_owned(), name.to_owned(), units.to_owned()))
    );
}

// POST /v0/projects/{project}/reports - the JMH and BenchmarkDotNet leaves create their new
// built-in measures and the measures they name at run time in their own units
//
// Kills: a new built-in measure missing from the server's built-ins, which creates it in the
// generic unit, and a measure a leaf names at run time created without the unit its harness gives.
#[tokio::test]
async fn reports_post_creates_the_jmh_and_dot_net_measures_in_their_units() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "jvmdotnet").await;
    for (adapter, results) in [
        ("java_jmh", JMH_MODES),
        ("java_jmh", JMH_GC),
        ("c_sharp_dot_net", DOT_NET_DIAGNOSERS),
    ] {
        post_report_body(
            &server,
            &token,
            &project_slug,
            &parse_report(adapter, &[results], Some(1)),
        )
        .await;
    }
    let project_id = get_project_id(&server, &project_slug);
    let measures = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq_any([
            "allocated",
            "exceptionfrequency",
            "gc-alloc-rate",
            "gen0-collects-per-1000-ops",
            "sample-time",
            "single-shot-time",
        ]))
        .order(schema::measure::slug)
        .select((
            schema::measure::slug,
            schema::measure::name,
            schema::measure::units,
        ))
        .load::<(String, String, String)>(&mut server.db_conn())
        .expect("Failed to load the measures");
    assert_eq!(
        measures,
        [
            ("allocated", "allocated", "bytes (B)"),
            ("exceptionfrequency", "ExceptionFrequency", "Count"),
            ("gc-alloc-rate", "gc.alloc.rate", "MB/sec"),
            (
                "gen0-collects-per-1000-ops",
                "Gen0 Collects per 1000 Operations",
                "collects / 1000 operations"
            ),
            ("sample-time", "Sample Time", "nanoseconds (ns)"),
            ("single-shot-time", "Single Shot Time", "nanoseconds (ns)"),
        ]
        .map(|(slug, name, units)| (slug.to_owned(), name.to_owned(), units.to_owned()))
    );
}

// POST /v0/projects/{project}/reports - a JMH payload whose only variant has a parameter past the
// limit is a report that warns of the skipped variant
//
// Kills: a leaf that returns no results when it skipped every variant, which refuses the report.
#[tokio::test]
async fn reports_post_warns_of_a_jmh_variant_past_the_parameter_limit() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "jmhlongparam").await;
    let report = post_report_body(
        &server,
        &token,
        &project_slug,
        &parse_report("java_jmh", &[JMH_LONG_PARAM_ONLY], Some(1)),
    )
    .await;
    assert_eq!(
        report.warnings,
        Some(vec![skipped(ReportWarningResource::Variant, 1)])
    );
}

// POST /v0/projects/{project}/reports - the Dart leaf creates cycles and page faults in their own
// units
//
// Kills: the page faults built-in missing from the server's built-ins, which creates it in the
// generic unit.
#[tokio::test]
async fn reports_post_creates_the_dart_measures_in_their_units() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "dart").await;
    post_report_body(
        &server,
        &token,
        &project_slug,
        &parse_report("dart_benchmark_harness", &[DART_EMITTERS], Some(1)),
    )
    .await;
    let project_id = get_project_id(&server, &project_slug);
    let measures = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq_any(["cycles", "page-faults"]))
        .order(schema::measure::slug)
        .select((
            schema::measure::slug,
            schema::measure::name,
            schema::measure::units,
        ))
        .load::<(String, String, String)>(&mut server.db_conn())
        .expect("Failed to load the measures");
    assert_eq!(
        measures,
        [
            ("cycles", "Cycles", "cycles"),
            ("page-faults", "Page Faults", "faults"),
        ]
        .map(|(slug, name, units)| (slug.to_owned(), name.to_owned(), units.to_owned()))
    );
}

// POST /v0/projects/{project}/reports - the hyperfine and asv leaves create their new built-in
// measures in their units
//
// Kills: a new built-in measure missing from the server's built-ins, which creates it in the
// generic unit.
#[tokio::test]
async fn reports_post_creates_the_hyperfine_and_asv_measures_in_their_units() {
    let server = TestServer::new().await;
    let (token, project_slug) = ingest_project(&server, "hyperfineasv").await;
    for (adapter, results) in [
        ("shell_hyperfine", HYPERFINE_COUNTERS),
        ("python_asv", ASV_PARAMS),
    ] {
        post_report_body(
            &server,
            &token,
            &project_slug,
            &parse_report(adapter, &[results], Some(1)),
        )
        .await;
    }
    let project_id = get_project_id(&server, &project_slug);
    let measures = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq_any([
            "branch-misses",
            "cache-misses",
            "cache-references",
            "memory",
            "peak-memory",
            "system-time",
            "user-time",
        ]))
        .order(schema::measure::slug)
        .select((
            schema::measure::slug,
            schema::measure::name,
            schema::measure::units,
        ))
        .load::<(String, String, String)>(&mut server.db_conn())
        .expect("Failed to load the measures");
    assert_eq!(
        measures,
        [
            ("branch-misses", "Branch Misses", "misses"),
            ("cache-misses", "Cache Misses", "misses"),
            ("cache-references", "Cache References", "references"),
            ("memory", "Memory", "bytes (B)"),
            ("peak-memory", "Peak Memory", "bytes (B)"),
            ("system-time", "System Time", "nanoseconds (ns)"),
            ("user-time", "User Time", "nanoseconds (ns)"),
        ]
        .map(|(slug, name, units)| (slug.to_owned(), name.to_owned(), units.to_owned()))
    );
}

const JMH_MODES: &str = include_str!("../../bencher_adapter/tool_output/java/jmh/modes.json");
const HYPERFINE_COUNTERS: &str =
    include_str!("../../bencher_adapter/tool_output/shell/hyperfine/v2_counters.json");
const ASV_PARAMS: &str = include_str!("../../bencher_adapter/tool_output/python/asv/params.txt");
const DART_EMITTERS: &str =
    include_str!("../../bencher_adapter/tool_output/dart/benchmark_harness/emitters.txt");
const JMH_LONG_PARAM_ONLY: &str =
    include_str!("../../bencher_adapter/tool_output/java/jmh/long_param_only.json");
const JMH_GC: &str = include_str!("../../bencher_adapter/tool_output/java/jmh/gc_1_36.json");
const DOT_NET_DIAGNOSERS: &str =
    include_str!("../../bencher_adapter/tool_output/c_sharp/dot_net/diagnosers.json");

const GOOGLE_MEMORY: &str =
    include_str!("../../bencher_adapter/tool_output/cpp/google/memory.json");
const GO_BENCHMEM: &str = include_str!("../../bencher_adapter/tool_output/go/bench/benchmem.txt");

const CRITERION_CYCLES_PER_BYTE: &str =
    include_str!("../../bencher_adapter/tool_output/rust/criterion/cycles_per_byte.txt");
const CRITERION_PERF_EVENTS: &str =
    include_str!("../../bencher_adapter/tool_output/rust/criterion/json_perf_events.txt");

/// A report of `results` at `bmf_version` that keeps only `measures`.
fn keeping(results: &serde_json::Value, bmf_version: u8, measures: &[&str]) -> serde_json::Value {
    settled_report(
        &results.to_string(),
        bmf_version,
        &serde_json::json!({ "adapter": "json", "measures": measures }),
    )
}

/// A report of `results` at `bmf_version` with `settings`.
fn settled_report(
    results: &str,
    bmf_version: u8,
    settings: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [results],
        "bmf_version": bmf_version,
        "settings": settings,
    })
}

/// `count` measures to keep: `throughput` first, `latency` last, and measures the results lack
/// between them.
fn listed_with_latency_last(count: usize) -> Vec<String> {
    std::iter::once("throughput".to_owned())
        .chain((1..count - 1).map(|n| format!("m{n}")))
        .chain(std::iter::once("latency".to_owned()))
        .collect()
}

/// The kept and dropped measures, and a benchmark with only a dropped one, at `bmf_version`.
fn kept_and_dropped(bmf_version: u8) -> serde_json::Value {
    let kept = serde_json::json!({
        "latency": { "value": 1.0 },
        "throughput": { "value": 2.0 },
        "my-kept": { "value": 3.0 },
        "my-dropped": { "value": 4.0 },
        "global-bus-events": { "value": 6.0 },
    });
    let dropped = serde_json::json!({ "my-dropped": { "value": 5.0 } });
    if bmf_version == 0 {
        serde_json::json!({ "bench_kept": kept, "bench_dropped": dropped })
    } else {
        serde_json::json!({
            "bench_kept": [{ "measures": kept }],
            "bench_dropped": [{ "measures": dropped }],
        })
    }
}

/// The slugs of a project's measures, its benchmarks' names, its report benchmarks, and the
/// series it bills, as Plus counts them.
fn stored(server: &TestServer, project_slug: &str) -> (Vec<String>, Vec<String>, i64, i64) {
    let project_id = get_project_id(server, project_slug);
    let mut conn = server.db_conn();
    let measures = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .order(schema::measure::slug)
        .select(schema::measure::slug)
        .load::<Slug>(&mut conn)
        .expect("Failed to load the measures")
        .into_iter()
        .map(|slug| slug.to_string())
        .collect();
    let benchmarks = schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::benchmark::name)
        .select(schema::benchmark::name)
        .load::<BenchmarkName>(&mut conn)
        .expect("Failed to load the benchmarks")
        .into_iter()
        .map(|name| name.to_string())
        .collect();
    let report_benchmarks = schema::report_benchmark::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .count()
        .get_result(&mut conn)
        .expect("Failed to count the report benchmarks");
    #[cfg(feature = "plus")]
    let series = schema::series_last_seen::table
        .filter(schema::series_last_seen::project_id.eq(project_id))
        .count()
        .get_result(&mut conn)
        .expect("Failed to count the series");
    #[cfg(not(feature = "plus"))]
    let series = 0;
    (measures, benchmarks, report_benchmarks, series)
}
