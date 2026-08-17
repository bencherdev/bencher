#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Benchmark parameters and named metric values, end to end through ingest.
//!
//! The promise this file exists to keep is that no existing project's alert volume
//! changes. Every other test here is about the new shape reaching the database;
//! [`alert_volume_is_identical_for_flat_benchmarks_and_grid_points`] is about the
//! old shape not moving when it does.

use bencher_api_tests::{TestServer, helpers::get_project_id};
use bencher_json::{JsonParameters, MetricName};
use bencher_schema::{context::DbConnection, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

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

/// A signed up user with an organization and a project to report into.
struct Fixture {
    project_slug: String,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("params{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Params Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Params Project {label}"))
        .await;
    Fixture {
        project_slug: project.slug.to_string(),
        token: user.token.clone(),
    }
}

/// Post one report of `results` and return the parsed response.
///
/// `day` only has to be distinct and increasing, so reports order the way they were
/// submitted. `thresholds` is passed on every report because the report endpoint
/// upserts it, which keeps the fixtures to a single request per step.
async fn report(
    server: &TestServer,
    fixture: &Fixture,
    day: usize,
    results: Vec<String>,
    thresholds: Option<serde_json::Value>,
    fold: Option<&str>,
) -> serde_json::Value {
    // Both optional fields are omitted by sending `null`, which is what an absent
    // `Option` deserializes from.
    let body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": format!("2024-01-{day:02}T00:00:00Z"),
        "end_time": format!("2024-01-{day:02}T00:01:00Z"),
        "results": results,
        "thresholds": thresholds,
        "settings": fold.map(|fold| serde_json::json!({ "fold": fold })),
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

/// Every parameter set stored under a project, with the number of `report_benchmark`
/// rows pointing at it.
fn parameter_sets(conn: &mut DbConnection, project_id: i32) -> Vec<(JsonParameters, i64)> {
    let parameters: Vec<(i32, JsonParameters)> = schema::parameter::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::parameter::id.asc())
        .select((schema::parameter::id, schema::parameter::parameters))
        .load(&mut *conn)
        .expect("Failed to load the parameter sets");

    parameters
        .into_iter()
        .map(|(parameter_id, parameters)| {
            let count: i64 = schema::report_benchmark::table
                .filter(schema::report_benchmark::parameter_id.eq(parameter_id))
                .count()
                .get_result(&mut *conn)
                .expect("Failed to count the report benchmarks");
            (parameters, count)
        })
        .collect()
}

/// Every metric name stored for a project, with its value, keyed by parameter set.
fn named_values(conn: &mut DbConnection, project_id: i32) -> Vec<(JsonParameters, String, f64)> {
    schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::parameter::table)
                .inner_join(schema::benchmark::table),
        )
        .filter(schema::benchmark::project_id.eq(project_id))
        .order((schema::parameter::id.asc(), schema::metric::name.asc()))
        .select((
            schema::parameter::parameters,
            schema::metric::name,
            schema::metric::value,
        ))
        .load::<(JsonParameters, MetricName, f64)>(&mut *conn)
        .expect("Failed to load the metric rows")
        .into_iter()
        .map(|(parameters, name, value)| (parameters, name.to_string(), value))
        .collect()
}

/// The parameter set and metric name of every alert in a project.
fn alerts(conn: &mut DbConnection, project_id: i32) -> Vec<(JsonParameters, String)> {
    schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::parameter::table)
                        .inner_join(schema::benchmark::table),
                ),
            ),
        )
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::alert::id.asc())
        .select((schema::parameter::parameters, schema::metric::name))
        .load::<(JsonParameters, MetricName)>(&mut *conn)
        .expect("Failed to load the alerts")
        .into_iter()
        .map(|(parameters, name)| (parameters, name.to_string()))
        .collect()
}

/// The metric name every boundary is attached to.
fn boundary_names(conn: &mut DbConnection, project_id: i32) -> Vec<String> {
    schema::boundary::table
        .inner_join(
            schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(schema::benchmark::table)),
        )
        .filter(schema::benchmark::project_id.eq(project_id))
        .select(schema::metric::name)
        .load::<MetricName>(&mut *conn)
        .expect("Failed to load the boundaries")
        .into_iter()
        .map(|name| name.to_string())
        .collect()
}

fn parameters(canonical: &str) -> JsonParameters {
    canonical.parse().expect("Failed to parse the parameters")
}

// A BMF v1 report lands one `report_benchmark` row per grid point and one `metric`
// row per named value, under the parameter set the entry declared.
#[tokio::test]
async fn v1_report_lands_grid_points_and_named_values() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "grid").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[
                entry(
                    &serde_json::json!({ "size_mb": 16 }),
                    &serde_json::json!({ "latency": { "value": 42.0, "p99": 97.0 } }),
                ),
                entry(
                    &serde_json::json!({ "size_mb": 32 }),
                    &serde_json::json!({ "latency": { "value": 55.0 } }),
                ),
            ],
        )],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    // The empty set the benchmark was born with, plus one row per grid point.
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![
            (JsonParameters::default(), 0),
            (parameters(r#"{"size_mb": 16}"#), 1),
            (parameters(r#"{"size_mb": 32}"#), 1),
        ],
    );

    assert_eq!(
        named_values(&mut conn, project_id),
        vec![
            (parameters(r#"{"size_mb": 16}"#), "p99".to_owned(), 97.0),
            (parameters(r#"{"size_mb": 16}"#), "value".to_owned(), 42.0),
            (parameters(r#"{"size_mb": 32}"#), "value".to_owned(), 55.0),
        ],
    );
}

/// The point estimates each grid point reports, run after run. The first grid point
/// jumps tenfold on the final report and the second does not.
const SMALL: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const LARGE: [f64; 5] = [100.0, 101.0, 102.0, 103.0, 104.0];
const SMALL_FINAL: f64 = 1_000.0;
const LARGE_FINAL: f64 = 104.0;

/// Ingest the same measurements as two flat benchmarks, and return the project.
async fn ingest_flat(server: &TestServer) -> (Fixture, i32) {
    let fixture = fixture(server, "flat").await;
    for (day, (small, large)) in SMALL
        .into_iter()
        .zip(LARGE)
        .chain([(SMALL_FINAL, LARGE_FINAL)])
        .enumerate()
    {
        let results = serde_json::json!({
            "bench_16": { "latency": { "value": small } },
            "bench_32": { "latency": { "value": large } },
        });
        report(
            server,
            &fixture,
            day + 1,
            vec![serde_json::to_string(&results).expect("the results serialize")],
            Some(threshold_models()),
            None,
        )
        .await;
    }
    let project_id = get_project_id(server, &fixture.project_slug);
    (fixture, project_id)
}

/// Ingest the same measurements as one benchmark's two grid points.
async fn ingest_grid(server: &TestServer) -> (Fixture, i32) {
    let fixture = fixture(server, "grid-parity").await;
    for (day, (small, large)) in SMALL
        .into_iter()
        .zip(LARGE)
        .chain([(SMALL_FINAL, LARGE_FINAL)])
        .enumerate()
    {
        report(
            server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[
                    entry(
                        &serde_json::json!({ "size_mb": 16 }),
                        &serde_json::json!({ "latency": { "value": small } }),
                    ),
                    entry(
                        &serde_json::json!({ "size_mb": 32 }),
                        &serde_json::json!({ "latency": { "value": large } }),
                    ),
                ],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }
    let project_id = get_project_id(server, &fixture.project_slug);
    (fixture, project_id)
}

// A project whose grid points are flat benchmarks and a project whose grid points
// are parameter sets raise exactly the same alerts from exactly the same numbers.
//
// This is the promise of the whole layer: a bare threshold gates the conventional
// value series of every parameter set under its measure, which is what a
// measure-level threshold over flat benchmarks has always done.
#[tokio::test]
async fn alert_volume_is_identical_for_flat_benchmarks_and_grid_points() {
    let server = TestServer::new().await;

    let (_flat, flat_project) = ingest_flat(&server).await;
    let (_grid, grid_project) = ingest_grid(&server).await;

    let mut conn = server.db_conn();
    let flat_alerts = alerts(&mut conn, flat_project);
    let grid_alerts = alerts(&mut conn, grid_project);

    assert_eq!(
        flat_alerts.len(),
        1,
        "the flat fixture raises exactly one alert, got {flat_alerts:?}"
    );
    assert_eq!(
        grid_alerts.len(),
        flat_alerts.len(),
        "migrating to parameters must not change alert volume"
    );
    assert_eq!(
        grid_alerts,
        vec![(parameters(r#"{"size_mb": 16}"#), "value".to_owned())],
        "the alert belongs to the grid point that regressed"
    );
}

/// Two tight histories, decades apart. The regression at the end of `TIGHT_SMALL`
/// is a large outlier against its own history and sits well inside a history pooled
/// with `TIGHT_LARGE`, whose spread swamps it.
///
/// That gap is the whole point of these numbers. The parity fixture above cannot
/// tell a per grid point baseline from a pooled one, because a tenfold jump is an
/// outlier either way; here a pooled baseline raises no alert at all.
const TIGHT_SMALL: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const TIGHT_LARGE: [f64; 5] = [1_000.0, 1_001.0, 1_002.0, 1_003.0, 1_004.0];
const TIGHT_SMALL_FINAL: f64 = 50.0;
const TIGHT_LARGE_FINAL: f64 = 1_004.0;

// One grid point's regression does not alert against another's baseline, and one
// grid point's ordinary run is not an outlier against the other's history.
//
// The historical query behind detection has to filter on the parameter set as well
// as the benchmark. Drop that filter and the two grid points pool into one sample
// whose standard deviation hides the regression, so this test raises no alert.
#[tokio::test]
async fn baselines_separate_by_parameter() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "baseline").await;

    for (day, (small, large)) in TIGHT_SMALL
        .into_iter()
        .zip(TIGHT_LARGE)
        .chain([(TIGHT_SMALL_FINAL, TIGHT_LARGE_FINAL)])
        .enumerate()
    {
        report(
            &server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[
                    entry(
                        &serde_json::json!({ "size_mb": 16 }),
                        &serde_json::json!({ "latency": { "value": small } }),
                    ),
                    entry(
                        &serde_json::json!({ "size_mb": 32 }),
                        &serde_json::json!({ "latency": { "value": large } }),
                    ),
                ],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();
    assert_eq!(
        alerts(&mut conn, project_id),
        vec![(parameters(r#"{"size_mb": 16}"#), "value".to_owned())],
        "each grid point is tested against its own history, and only the small one regressed"
    );
}

// A bare threshold gates the conventional `value` series and nothing else: a report
// carrying `p50` and `p99` produces boundaries on `value` alone.
#[tokio::test]
async fn bare_threshold_gates_only_the_value_name() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "named").await;

    for (day, value) in SMALL.into_iter().chain([SMALL_FINAL]).enumerate() {
        report(
            &server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({}),
                    &serde_json::json!({
                        "latency": { "value": value, "p50": value - 1.0, "p99": value + 1.0 }
                    }),
                )],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    let names = boundary_names(&mut conn, project_id);
    assert!(!names.is_empty(), "the fixture computes boundaries");
    assert!(
        names.iter().all(|name| name == "value"),
        "a bare threshold gates only the point estimate, got {names:?}"
    );

    let alerts = alerts(&mut conn, project_id);
    assert_eq!(
        alerts,
        vec![(JsonParameters::default(), "value".to_owned())],
        "the alert is on the point estimate"
    );
}

// An entry with no `parameters` lands on the benchmark's empty parameter set,
// which is the same set an explicit `{}` lands on.
#[tokio::test]
async fn absent_parameters_land_on_the_empty_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "absent").await;

    report(
        &server,
        &fixture,
        1,
        vec![
            serde_json::to_string(&serde_json::json!({
                "bench": [
                    { "measures": { "latency": { "value": 1.0 } } },
                    { "parameters": {}, "measures": { "throughput": { "value": 2.0 } } },
                ]
            }))
            .expect("the results serialize"),
        ],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(JsonParameters::default(), 1)],
        "an absent parameter set and an explicit empty one are one grid point"
    );
}

// An archived parameter set that reports again is unarchived, exactly as an
// archived benchmark is.
#[tokio::test]
async fn archived_parameter_set_is_unarchived_on_report() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "archived").await;

    let grid_point = v1(
        "bench",
        &[entry(
            &serde_json::json!({ "size_mb": 16 }),
            &serde_json::json!({ "latency": { "value": 1.0 } }),
        )],
    );
    report(&server, &fixture, 1, vec![grid_point.clone()], None, None).await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    {
        let mut conn = server.db_conn();
        let updated = diesel::update(
            schema::parameter::table
                .filter(schema::parameter::parameters.eq(parameters(r#"{"size_mb": 16}"#))),
        )
        .set(schema::parameter::archived.eq(Some(1_000_000_000i64)))
        .execute(&mut conn)
        .expect("Failed to archive the parameter set");
        assert_eq!(updated, 1);
    }

    report(&server, &fixture, 2, vec![grid_point], None, None).await;

    let mut conn = server.db_conn();
    let archived: Vec<Option<i64>> = schema::parameter::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .filter(schema::parameter::parameters.eq(parameters(r#"{"size_mb": 16}"#)))
        .select(schema::parameter::archived)
        .load(&mut conn)
        .expect("Failed to load the parameter set");
    assert_eq!(archived, vec![None], "reporting unarchives the grid point");
}

// Fold is not supported for BMF v1: the report warns and ingests unfolded, one
// `report_benchmark` row per iteration, and never errors.
#[tokio::test]
async fn fold_with_v1_warns_and_lands_every_iteration() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "fold").await;

    report(
        &server,
        &fixture,
        1,
        vec![
            v1(
                "bench",
                &[entry(
                    &serde_json::json!({}),
                    &serde_json::json!({ "latency": { "value": 10.0 } }),
                )],
            ),
            v1(
                "bench",
                &[entry(
                    &serde_json::json!({}),
                    &serde_json::json!({ "latency": { "value": 20.0 } }),
                )],
            ),
        ],
        None,
        Some("min"),
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    let iterations: Vec<i32> = schema::report_benchmark::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::report_benchmark::iteration.asc())
        .select(schema::report_benchmark::iteration)
        .load(&mut conn)
        .expect("Failed to load the report benchmarks");
    assert_eq!(iterations, vec![0, 1], "a v1 payload ingests unfolded");

    let values: Vec<f64> = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::benchmark::table))
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::metric::value.asc())
        .select(schema::metric::value)
        .load(&mut conn)
        .expect("Failed to load the metric rows");
    assert_eq!(values, vec![10.0, 20.0], "nothing was folded away");
}

// The named value cap drops the excess best effort. The report still succeeds.
#[tokio::test]
async fn named_value_cap_does_not_fail_the_report() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "cap").await;

    let mut measures = serde_json::Map::new();
    measures.insert("value".to_owned(), serde_json::json!(1.0));
    for index in 0..9u32 {
        measures.insert(format!("p{index}"), serde_json::json!(f64::from(index)));
    }

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({ "latency": measures }),
            )],
        )],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();
    let names = named_values(&mut conn, project_id);
    assert_eq!(names.len(), 8, "the cap keeps eight names, got {names:?}");
    assert!(
        names.iter().any(|(_, name, _)| name == "value"),
        "the point estimate is never dropped, got {names:?}"
    );
}

// What the legacy metric-count meter counts once named metric values exist.
//
// `QueryMetric::usage` counts `value` rows, so a measure carrying several names
// still counts once: named values do not inflate a metric count. The other half is
// the open question this test records rather than settles: a measure that names no
// `value` at all, which only BMF v1 can produce, counts zero.
#[tokio::test]
async fn named_values_do_not_change_the_metric_count() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "meter").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({
                    // Four names, one measurement.
                    "latency": { "value": 1.0, "lower_value": 0.5, "upper_value": 1.5, "p99": 2.0 },
                    // No point estimate at all.
                    "throughput": { "p99": 3.0 },
                }),
            )],
        )],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    let rows = named_values(&mut conn, project_id);
    assert_eq!(rows.len(), 5, "five named rows landed, got {rows:?}");

    let billable: i64 = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::benchmark::table))
        .filter(schema::benchmark::project_id.eq(project_id))
        .filter(schema::metric::name.eq(MetricName::value()))
        .count()
        .get_result(&mut conn)
        .expect("Failed to count the billable metric rows");
    assert_eq!(
        billable, 1,
        "the four named latency values bill as one metric, and the throughput measure \
         that named no point estimate bills as none"
    );

    // The in-request count the plan check and the telemetry counter see agrees with
    // what the billing read counts back, so the two meters cannot drift.
    let counted: i32 = schema::metric_count_by_report::table
        .select(schema::metric_count_by_report::metric_count)
        .first(&mut conn)
        .expect("Failed to read the report's metric count");
    assert_eq!(i64::from(counted), billable);
}

// The report response echoes every named value, keeps the deprecated metric,
// threshold, and boundary fields correct, and separates two grid points into two
// results rather than merging them into one.
//
// Two measures across two parameter sets is the smallest fixture that can see the
// results query's ordering. Drop the parameter from the `ORDER BY` and the measure
// name outranks it, so the rows arrive interleaved by grid point and the grouping,
// which only ever compares against the previous row, emits four results of one
// measure each instead of two results of two measures each.
#[tokio::test]
async fn report_response_echoes_named_values_and_separates_grid_points() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "response").await;

    // Enough history for the final report to carry a boundary.
    for (day, value) in SMALL.into_iter().enumerate() {
        report(
            &server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[
                    entry(
                        &serde_json::json!({ "size_mb": 16 }),
                        &serde_json::json!({
                            "latency": { "value": value },
                            "throughput": { "value": value * 2.0 },
                        }),
                    ),
                    entry(
                        &serde_json::json!({ "size_mb": 32 }),
                        &serde_json::json!({
                            "latency": { "value": value * 10.0 },
                            "throughput": { "value": value * 20.0 },
                        }),
                    ),
                ],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let response = report(
        &server,
        &fixture,
        SMALL.len() + 1,
        vec![v1(
            "bench",
            &[
                entry(
                    &serde_json::json!({ "size_mb": 16 }),
                    &serde_json::json!({
                        "latency": { "value": 12.0, "lower_value": 11.0, "upper_value": 13.0, "p99": 20.0 },
                        "throughput": { "value": 24.0 },
                    }),
                ),
                entry(
                    &serde_json::json!({ "size_mb": 32 }),
                    &serde_json::json!({
                        "latency": { "value": 120.0 },
                        "throughput": { "value": 240.0 },
                    }),
                ),
            ],
        )],
        Some(threshold_models()),
        None,
    )
    .await;

    let results = response
        .pointer("/results/0")
        .and_then(serde_json::Value::as_array)
        .expect("the report echoes its results");
    assert_eq!(
        results.len(),
        2,
        "two grid points are two results, got {results:#?}"
    );

    // Each grid point keeps both of its measures together in one result. Four
    // results of one measure each is what a benchmark first ordering produces.
    let measure_names: Vec<Vec<&str>> = results
        .iter()
        .map(|result| {
            result
                .pointer("/measures")
                .and_then(serde_json::Value::as_array)
                .expect("each result echoes its measures")
                .iter()
                .map(|measure| {
                    measure
                        .pointer("/measure/slug")
                        .and_then(serde_json::Value::as_str)
                        .expect("each measure has a slug")
                })
                .collect()
        })
        .collect();
    assert_eq!(
        measure_names,
        vec![vec!["latency", "throughput"], vec!["latency", "throughput"]],
        "both measures of a grid point belong to that grid point's one result"
    );

    // The counts are computed two ways, from the loaded results and by aggregate
    // query, and both have to see two grid points rather than one benchmark.
    assert_eq!(
        response.pointer("/counts/results/0"),
        Some(&serde_json::json!({ "benchmarks": 2, "measures": 2 })),
    );

    let grid_points: Vec<JsonParameters> = results
        .iter()
        .map(|result| {
            serde_json::from_value(
                result
                    .pointer("/parameter/parameters")
                    .expect("each result names its parameter set")
                    .clone(),
            )
            .expect("the parameter set parses")
        })
        .collect();
    assert_eq!(
        grid_points,
        vec![
            parameters(r#"{"size_mb": 16}"#),
            parameters(r#"{"size_mb": 32}"#),
        ],
    );

    let measure = results[0]
        .pointer("/measures/0")
        .expect("the result echoes its measure");

    // The named values, in a stable order.
    let names: Vec<&str> = measure
        .pointer("/metrics")
        .and_then(serde_json::Value::as_array)
        .expect("the measure echoes its named values")
        .iter()
        .map(|metric| {
            metric
                .pointer("/name")
                .and_then(serde_json::Value::as_str)
                .expect("each named value has a name")
        })
        .collect();
    assert_eq!(names, vec!["lower_value", "p99", "upper_value", "value"]);

    // The deprecated trio, reconstructed from the `value` row and its siblings.
    assert_eq!(
        measure.pointer("/metric/value"),
        Some(&serde_json::json!(12.0))
    );
    assert_eq!(
        measure.pointer("/metric/lower_value"),
        Some(&serde_json::json!(11.0))
    );
    assert_eq!(
        measure.pointer("/metric/upper_value"),
        Some(&serde_json::json!(13.0))
    );
    assert!(
        measure
            .pointer("/threshold")
            .is_some_and(|threshold| !threshold.is_null()),
        "the deprecated threshold is populated, got {measure:#?}"
    );
    assert!(
        measure
            .pointer("/boundary")
            .is_some_and(|boundary| !boundary.is_null()),
        "the deprecated boundary is populated, got {measure:#?}"
    );

    // The plural form pairs each threshold with the boundary it produced, and only
    // the point estimate was gated.
    let boundaries = |name: &str| -> usize {
        measure
            .pointer("/metrics")
            .and_then(serde_json::Value::as_array)
            .expect("the measure echoes its named values")
            .iter()
            .find(|metric| metric.pointer("/name") == Some(&serde_json::json!(name)))
            .and_then(|metric| metric.pointer("/boundaries"))
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len)
    };
    assert_eq!(boundaries("value"), 1, "the point estimate was gated");
    assert_eq!(boundaries("p99"), 0, "a named value was not gated");
}
