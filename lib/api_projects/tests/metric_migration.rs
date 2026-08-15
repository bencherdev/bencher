#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The single-valued metric migration, pinned by captured response bytes.
//!
//! A migration that rebuilds the metric table is only safe if it is invisible from
//! the outside. That is not a claim to assert, it is a thing to demonstrate: these
//! tests seed a database in the shape the migration finds, capture the raw bytes of
//! every response that reads a metric, run the migration, and capture them again.
//! The two captures must be equal byte for byte.
//!
//! The same binary serves both captures because every reader goes through the
//! `metric_boundary` view, whose column list the migration holds unchanged.

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, create_empty_parameter, get_project_id},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, HeadUuid, MeasureUuid, MetricName,
    MetricUuid, ModelUuid, ReportBenchmarkUuid, ReportUuid, TestbedUuid, ThresholdUuid,
    VersionUuid, project::alert::AlertStatus, project::boundary::BoundaryLimit,
};
use bencher_schema::{MIGRATIONS, context::DbConnection, schema};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    connection::SimpleConnection as _,
    sql_types::{Double, Integer, Nullable, Text},
};
use diesel_migrations::MigrationHarness as _;
use http::StatusCode;

/// A metric as the pre-migration table held it: one row, three columns.
#[derive(Debug, Clone, Copy)]
struct LegacyMetric {
    value: f64,
    lower_value: Option<f64>,
    upper_value: Option<f64>,
}

/// The seeded project, and every handle a request needs to reach its metrics.
struct Fixture {
    project_slug: String,
    token: String,
    branch_uuid: BranchUuid,
    testbed_uuid: TestbedUuid,
    benchmark_uuid: BenchmarkUuid,
    measure_uuids: Vec<MeasureUuid>,
    report_uuid: ReportUuid,
    metric_uuids: Vec<MetricUuid>,
}

/// The raw bytes of every response that reads a metric.
#[derive(Debug, PartialEq, Eq)]
struct Captured {
    perf: String,
    metrics: Vec<String>,
    report: String,
    alerts: String,
}

/// The fixture rows, chosen to reach every branch of the explosion and the pivot.
///
/// Both bounds, lower only, upper only, and neither all appear, spread over two
/// measures and two report benchmarks so that a pivot joining on the wrong key
/// leaks a bound across a row and fails. The floats are the ones that expose
/// formatting drift: a value with no exact binary representation, a value that
/// only survives a round trip with full precision, a subnormal-adjacent exponent,
/// and negative zero, whose sign is invisible to `==` but not to `to_string`.
const FIXTURE_METRICS: [LegacyMetric; 4] = [
    LegacyMetric {
        value: 42.0,
        lower_value: Some(40.5),
        upper_value: Some(44.25),
    },
    LegacyMetric {
        value: 0.1,
        lower_value: None,
        upper_value: Some(0.300_000_000_000_000_04),
    },
    LegacyMetric {
        value: 1e-7,
        lower_value: Some(5.000_000_000_000_001e-8),
        upper_value: None,
    },
    LegacyMetric {
        value: -0.0,
        lower_value: None,
        upper_value: None,
    },
];

// Every response that reads a metric is byte identical across the migration.
#[tokio::test]
async fn responses_are_byte_identical_across_the_migration() {
    let server = TestServer::new().await;
    let fixture = seed_legacy_project(&server, "equiv").await;

    let before = capture(&server, &fixture).await;
    assert!(
        before.perf.contains("44.25") && before.alerts.contains("40.5"),
        "the fixture reaches the perf and alert responses before anything is compared"
    );
    apply_migration(&server);
    let after = capture(&server, &fixture).await;

    assert_eq!(
        before.perf, after.perf,
        "the perf response changed across the migration"
    );
    assert_eq!(
        before.metrics, after.metrics,
        "a metric response changed across the migration"
    );
    assert_eq!(
        before.report, after.report,
        "the report response changed across the migration"
    );
    assert_eq!(
        before.alerts, after.alerts,
        "the alerts response changed across the migration"
    );
}

// The migration explodes each metric into its named rows, keeping the point
// estimate's identity so that no boundary row has to be rewritten.
#[tokio::test]
async fn migration_explodes_the_metric_triple_into_named_rows() {
    let server = TestServer::new().await;
    let _fixture = seed_legacy_project(&server, "explode").await;

    let mut conn = server.db_conn();
    let legacy_ids: Vec<(i32, String)> = schema::metric::table
        .order(schema::metric::id)
        .select((schema::metric::id, schema::metric::uuid))
        .load(&mut conn)
        .expect("Failed to read the legacy metric rows");
    let boundary_metric_ids: Vec<i32> = schema::boundary::table
        .order(schema::boundary::id)
        .select(schema::boundary::metric_id)
        .load(&mut conn)
        .expect("Failed to read the boundary rows");
    drop(conn);

    apply_migration(&server);

    let mut conn = server.db_conn();
    let value_ids: Vec<(i32, String)> = schema::metric::table
        .filter(schema::metric::name.eq(MetricName::value()))
        .order(schema::metric::id)
        .select((schema::metric::id, schema::metric::uuid))
        .load(&mut conn)
        .expect("Failed to read the value rows");
    assert_eq!(
        value_ids, legacy_ids,
        "every value row keeps the id and uuid of the row it came from"
    );

    let expected_rows = FIXTURE_METRICS
        .iter()
        .map(|metric| {
            1 + usize::from(metric.lower_value.is_some())
                + usize::from(metric.upper_value.is_some())
        })
        .sum::<usize>();
    let rows: i64 = schema::metric::table
        .count()
        .get_result(&mut conn)
        .expect("Failed to count the metric rows");
    assert_eq!(
        usize::try_from(rows).expect("row count fits"),
        expected_rows,
        "each stored bound becomes its own row and nothing else does"
    );

    for (name, bound) in [
        (MetricName::lower_value(), 0),
        (MetricName::upper_value(), 1),
    ] {
        let values: Vec<f64> = schema::metric::table
            .filter(schema::metric::name.eq(name.clone()))
            .order(schema::metric::id)
            .select(schema::metric::value)
            .load(&mut conn)
            .expect("Failed to read the bound rows");
        let expected: Vec<f64> = FIXTURE_METRICS
            .iter()
            .filter_map(|metric| {
                if bound == 0 {
                    metric.lower_value
                } else {
                    metric.upper_value
                }
            })
            .collect();
        assert_eq!(values, expected, "the {name} rows carry the stored bounds");
    }

    let remapped: Vec<i32> = schema::boundary::table
        .order(schema::boundary::id)
        .select(schema::boundary::metric_id)
        .load(&mut conn)
        .expect("Failed to read the boundary rows");
    assert_eq!(
        remapped, boundary_metric_ids,
        "no boundary row is rewritten: it already points at the value row"
    );

    let named_uuids: Vec<String> = schema::metric::table
        .select(schema::metric::uuid)
        .load(&mut conn)
        .expect("Failed to read the metric uuids");
    let mut unique = named_uuids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        named_uuids.len(),
        "every minted uuid is distinct"
    );
    for uuid in &named_uuids {
        assert!(
            uuid.parse::<uuid::Uuid>().is_ok(),
            "minted uuid {uuid} is a uuid"
        );
    }
}

// Reverting and re-applying the migration is a round trip: the metric triple that
// goes down comes back up unchanged.
#[tokio::test]
async fn migration_down_and_up_round_trips_the_metric_triple() {
    let server = TestServer::new().await;
    let fixture = seed_legacy_project(&server, "roundtrip").await;
    apply_migration(&server);

    let after_first = capture(&server, &fixture).await;

    let mut conn = server.db_conn();
    revert_migration(&mut conn);
    drop(conn);
    apply_migration(&server);

    let after_round_trip = capture(&server, &fixture).await;
    assert_eq!(
        after_first, after_round_trip,
        "a down and up round trip leaves every response unchanged"
    );
}

// Ingest writes the metric triple as named rows, and the report it answers with is
// the report the single row shape answered with.
#[tokio::test]
async fn ingest_writes_the_metric_triple_as_named_rows() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "metricingest@example.com").await;
    let org = server.create_org(&user, "Metric Ingest Org").await;
    let project = server
        .create_project(&user, &org, "Metric Ingest Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": [
                "{\"bench_one\": {\"latency\": {\"value\": 100.0, \"lower_value\": 90.5, \"upper_value\": 110.25}}, \"bench_two\": {\"latency\": {\"value\": 200.0}}}"
            ]
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: serde_json::Value = resp.json().await.expect("Failed to parse the report");

    let bounded = report
        .pointer("/results/0/0/measures/0/metric")
        .expect("the report echoes its metric");
    assert_eq!(
        bounded
            .get("lower_value")
            .and_then(serde_json::Value::as_f64),
        Some(90.5),
        "the ingested lower bound comes back on the report"
    );
    assert_eq!(
        bounded
            .get("upper_value")
            .and_then(serde_json::Value::as_f64),
        Some(110.25),
        "the ingested upper bound comes back on the report"
    );

    let mut conn = server.db_conn();
    let mut named: Vec<(String, f64)> = schema::metric::table
        .select((schema::metric::name, schema::metric::value))
        .load::<(MetricName, f64)>(&mut conn)
        .expect("Failed to read the metric rows")
        .into_iter()
        .map(|(name, value)| (name.as_ref().to_owned(), value))
        .collect();
    named.sort_by(|left, right| left.partial_cmp(right).expect("fixture floats are ordered"));
    assert_eq!(
        named,
        vec![
            ("lower_value".to_owned(), 90.5),
            ("upper_value".to_owned(), 110.25),
            ("value".to_owned(), 100.0),
            ("value".to_owned(), 200.0),
        ],
        "the bounded metric writes three rows and the bare metric writes one"
    );
}

/// Run the single valued metric migration on the test server's database.
///
/// Foreign keys cannot be toggled inside a transaction and Diesel runs each
/// migration in one, so they are disabled around it, exactly as the API server
/// disables them around startup migrations. Without that, recreating `metric`
/// would cascade its boundaries away.
fn apply_migration(server: &TestServer) {
    let mut conn = server.db_conn();
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to apply the single valued metric migration");
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

/// Revert the single valued metric migration on the test server's database.
fn revert_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    conn.revert_last_migration(MIGRATIONS)
        .expect("Failed to revert the single valued metric migration");
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

/// Seed a project whose metrics are stored in the pre-migration shape.
///
/// The migration is reverted before the metric rows go in, so they are written
/// through raw SQL against the columns that shape had. Everything above the metric
/// table is untouched by this migration and is seeded normally.
async fn seed_legacy_project(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("metric{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Metric Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Metric Project {label}"))
        .await;
    let project_slug = project.slug.to_string();
    let project_id = get_project_id(server, &project_slug);

    let mut conn = server.db_conn();
    revert_migration(&mut conn);

    let now = base_timestamp();

    let testbed_uuid = TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq("metric-testbed"),
            schema::testbed::slug.eq(format!("metric-testbed-{testbed_uuid}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the testbed");
    let testbed_id: i32 = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&testbed_uuid))
        .select(schema::testbed::id)
        .first(&mut conn)
        .expect("Failed to get the testbed id");

    let version_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(&version_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the version");
    let version_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(&version_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("Failed to get the version id");

    let branch_uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(&branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq("metric-main"),
            schema::branch::slug.eq(format!("metric-main-{branch_uuid}")),
            schema::branch::created.eq(&now),
            schema::branch::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the branch");
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(&branch_uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to get the branch id");

    let head_uuid = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(&head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the head");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(&head_uuid))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("Failed to get the head id");

    // The perf query, given no explicit head, filters on the branch's current head.
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(&mut conn)
        .expect("Failed to set the branch head");

    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut conn)
        .expect("Failed to link the head to the version");

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
        .expect("Failed to insert the report");
    let report_id: i32 = schema::report::table
        .filter(schema::report::uuid.eq(&report_uuid))
        .select(schema::report::id)
        .first(&mut conn)
        .expect("Failed to get the report id");

    let benchmark_uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark_uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("metric-benchmark"),
            schema::benchmark::slug.eq(format!("metric-benchmark-{benchmark_uuid}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark_uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("Failed to get the benchmark id");
    let parameter_id = create_empty_parameter(&mut conn, benchmark_id);

    // Two measures, so a pivot that joins on the report benchmark alone pulls a
    // bound across measures and fails.
    let mut measure_uuids = Vec::new();
    let mut measure_ids = Vec::new();
    for (name, units) in [("metric-latency", "ns"), ("metric-throughput", "ops")] {
        let measure_uuid = MeasureUuid::new();
        diesel::insert_into(schema::measure::table)
            .values((
                schema::measure::uuid.eq(&measure_uuid),
                schema::measure::project_id.eq(project_id),
                schema::measure::name.eq(name),
                schema::measure::slug.eq(format!("{name}-{measure_uuid}")),
                schema::measure::units.eq(units),
                schema::measure::created.eq(&now),
                schema::measure::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert the measure");
        let measure_id: i32 = schema::measure::table
            .filter(schema::measure::uuid.eq(&measure_uuid))
            .select(schema::measure::id)
            .first(&mut conn)
            .expect("Failed to get the measure id");
        measure_uuids.push(measure_uuid);
        measure_ids.push(measure_id);
    }

    // Two report benchmarks, so a pivot that joins on the measure alone pulls a
    // bound across iterations and fails. Every (report benchmark, measure) pair
    // takes one fixture metric, in order.
    let mut grid = Vec::new();
    for iteration in 0..2 {
        let report_benchmark_uuid = ReportBenchmarkUuid::new();
        diesel::insert_into(schema::report_benchmark::table)
            .values((
                schema::report_benchmark::uuid.eq(&report_benchmark_uuid),
                schema::report_benchmark::report_id.eq(report_id),
                schema::report_benchmark::iteration.eq(iteration),
                schema::report_benchmark::benchmark_id.eq(benchmark_id),
                schema::report_benchmark::parameter_id.eq(parameter_id),
            ))
            .execute(&mut conn)
            .expect("Failed to insert the report benchmark");
        let report_benchmark_id: i32 = schema::report_benchmark::table
            .filter(schema::report_benchmark::uuid.eq(&report_benchmark_uuid))
            .select(schema::report_benchmark::id)
            .first(&mut conn)
            .expect("Failed to get the report benchmark id");
        for measure_id in &measure_ids {
            grid.push((report_benchmark_id, *measure_id));
        }
    }

    // The metrics go in through raw SQL: the columns they are written to are the
    // columns the pre-migration table had, which the compiled schema no longer names.
    let mut metric_uuids = Vec::new();
    for ((report_benchmark_id, measure_id), metric) in grid.into_iter().zip(FIXTURE_METRICS) {
        let metric_uuid = MetricUuid::new();
        diesel::sql_query(
            "INSERT INTO metric (uuid, report_benchmark_id, measure_id, value, lower_value, upper_value) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(metric_uuid.to_string())
        .bind::<Integer, _>(report_benchmark_id)
        .bind::<Integer, _>(measure_id)
        .bind::<Double, _>(metric.value)
        .bind::<Nullable<Double>, _>(metric.lower_value)
        .bind::<Nullable<Double>, _>(metric.upper_value)
        .execute(&mut conn)
        .expect("Failed to insert the legacy metric");
        metric_uuids.push(metric_uuid);
    }

    // The first fixture metric, the bounded one, gets the boundary and the alert.
    let (first_measure_id, first_metric_uuid) = measure_ids
        .first()
        .copied()
        .zip(metric_uuids.first())
        .expect("the fixture seeds at least one metric");
    seed_threshold_and_alert(
        &mut conn,
        project_id,
        branch_id,
        testbed_id,
        first_measure_id,
        first_metric_uuid,
        now,
    );

    Fixture {
        project_slug,
        token: user.token,
        branch_uuid,
        testbed_uuid,
        benchmark_uuid,
        measure_uuids,
        report_uuid,
        metric_uuids,
    }
}

/// Attach a threshold, model, boundary, and alert to the first seeded metric, so
/// that the boundary remap and the alert response are both exercised.
fn seed_threshold_and_alert(
    conn: &mut DbConnection,
    project_id: i32,
    branch_id: i32,
    testbed_id: i32,
    measure_id: i32,
    metric_uuid: &MetricUuid,
    now: bencher_json::DateTime,
) {
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
        .expect("Failed to insert the threshold");
    let threshold_id: i32 = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&threshold_uuid))
        .select(schema::threshold::id)
        .first(&mut *conn)
        .expect("Failed to get the threshold id");

    let model_uuid = ModelUuid::new();
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(&model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(0),
            schema::model::created.eq(&now),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the model");
    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(&model_uuid))
        .select(schema::model::id)
        .first(&mut *conn)
        .expect("Failed to get the model id");
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(&mut *conn)
        .expect("Failed to set the threshold model");

    let metric_id: i32 = schema::metric::table
        .filter(schema::metric::uuid.eq(metric_uuid))
        .select(schema::metric::id)
        .first(&mut *conn)
        .expect("Failed to get the metric id");

    let boundary_uuid = BoundaryUuid::new();
    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(&boundary_uuid),
            schema::boundary::metric_id.eq(metric_id),
            schema::boundary::threshold_id.eq(threshold_id),
            schema::boundary::model_id.eq(model_id),
            schema::boundary::baseline.eq(Some(41.0)),
            schema::boundary::lower_limit.eq(Some(39.0)),
            schema::boundary::upper_limit.eq(Some(43.0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the boundary");
    let boundary_id: i32 = schema::boundary::table
        .filter(schema::boundary::uuid.eq(&boundary_uuid))
        .select(schema::boundary::id)
        .first(&mut *conn)
        .expect("Failed to get the boundary id");

    let alert_uuid = AlertUuid::new();
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(&alert_uuid),
            schema::alert::boundary_id.eq(boundary_id),
            schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
            schema::alert::status.eq(AlertStatus::Active),
            schema::alert::modified.eq(&now),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the alert");
}

/// Capture the raw bytes of every response that reads a metric.
///
/// The bytes are compared, not parsed values: parsing and re-serializing would
/// hide exactly the float formatting drift this migration has to rule out.
async fn capture(server: &TestServer, fixture: &Fixture) -> Captured {
    let Fixture {
        project_slug,
        token,
        branch_uuid,
        testbed_uuid,
        benchmark_uuid,
        measure_uuids,
        report_uuid,
        metric_uuids,
    } = fixture;

    let measures = measure_uuids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let perf = get(
        server,
        token,
        &format!(
            "/v0/projects/{project_slug}/perf?branches={branch_uuid}&testbeds={testbed_uuid}&benchmarks={benchmark_uuid}&measures={measures}"
        ),
    )
    .await;

    let mut metrics = Vec::new();
    for metric_uuid in metric_uuids {
        metrics.push(
            get(
                server,
                token,
                &format!("/v0/projects/{project_slug}/metrics/{metric_uuid}"),
            )
            .await,
        );
    }

    let report = get(
        server,
        token,
        &format!("/v0/projects/{project_slug}/reports/{report_uuid}"),
    )
    .await;
    let alerts = get(
        server,
        token,
        &format!("/v0/projects/{project_slug}/alerts"),
    )
    .await;

    Captured {
        perf,
        metrics,
        report,
        alerts,
    }
}

/// GET a path and return its body, asserting that it was served.
async fn get(server: &TestServer, token: &str, path: &str) -> String {
    let resp = server
        .client
        .get(server.api_url(path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET {path}");
    resp.text().await.expect("Failed to read the response body")
}
