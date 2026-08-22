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

use bencher_api_tests::{
    TestServer,
    helpers::{base_timestamp, get_project_id},
};
use bencher_json::{DateTime, JsonParameters, MetricName, Slug};
use bencher_schema::{
    context::DbConnection,
    model::{organization::OrganizationId, project::metric::QueryMetric},
    schema,
};
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
    let (status, body) = try_report(server, fixture, day, results, thresholds, fold).await;
    assert_eq!(status, StatusCode::CREATED, "POST report {day}: {body}");
    serde_json::from_str(&body).expect("Failed to parse the report")
}

/// Post one report and return its status and body, whatever they are.
async fn try_report(
    server: &TestServer,
    fixture: &Fixture,
    day: usize,
    results: Vec<String>,
    thresholds: Option<serde_json::Value>,
    fold: Option<&str>,
) -> (StatusCode, String) {
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
    (status, body)
}

/// One BMF v1 payload for a single benchmark's grid points.
fn v1(benchmark: &str, entries: &[serde_json::Value]) -> String {
    serde_json::to_string(&serde_json::json!({ benchmark: entries }))
        .expect("the results serialize")
}

fn entry(parameters: &serde_json::Value, measures: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "parameters": parameters, "measures": measures })
}

/// The organization a project bills to, which is what the metric meter is keyed on.
fn organization_id(conn: &mut DbConnection, project_id: i32) -> OrganizationId {
    schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(conn)
        .expect("Failed to get the organization ID")
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

/// The measure of every billable series a project has, in slug order.
///
/// A series is `(testbed, benchmark, parameter, measure)`, so this is the measure
/// side of what the active series cache bills.
fn series_measures(conn: &mut DbConnection, project_id: i32) -> Vec<String> {
    schema::series_last_seen::table
        .inner_join(schema::measure::table)
        .filter(schema::series_last_seen::project_id.eq(project_id))
        .order(schema::measure::slug.asc())
        .select(schema::measure::slug)
        .load::<Slug>(&mut *conn)
        .expect("Failed to load the billable series")
        .into_iter()
        .map(|slug| slug.to_string())
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

/// The number of benchmarks a project has, so a test can say the benchmark was
/// still born even though nothing was measured under it.
fn benchmark_count(conn: &mut DbConnection, project_id: i32) -> i64 {
    schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(project_id))
        .count()
        .get_result(conn)
        .expect("Failed to count the benchmarks")
}

/// Every report's in-request metric count, which is what the plan check bills on.
fn metric_counts(conn: &mut DbConnection) -> Vec<i32> {
    schema::metric_count_by_report::table
        .select(schema::metric_count_by_report::metric_count)
        .load(conn)
        .expect("Failed to load the metric counts")
}

// A BMF v1 entry that names no measure measured nothing, so it costs nothing: no
// parameter set is minted for it, no `report_benchmark` row is written, and no
// series is billed. The parameter set is the sharp half. An entry declaring
// `{"size_mb": 16}` and measuring nothing must not leave a `{"size_mb": 16}` row
// behind, or a payload of empty entries is a free way to fill the parameter table.
//
// The benchmark itself is still born, with the empty parameter set every benchmark
// is born with, because the name was reported.
#[tokio::test]
async fn v1_entry_without_measures_mints_nothing() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "no-measures").await;

    let response = report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[
                entry(
                    &serde_json::json!({ "size_mb": 16 }),
                    &serde_json::json!({}),
                ),
                entry(&serde_json::json!({}), &serde_json::json!({})),
            ],
        )],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(JsonParameters::default(), 0)],
        "only the empty set the benchmark was born with, pointed at by nothing"
    );
    assert_eq!(named_values(&mut conn, project_id), vec![]);
    assert_eq!(series_measures(&mut conn, project_id), Vec::<String>::new());
    assert_eq!(metric_counts(&mut conn), vec![0]);
    assert_eq!(
        response.get("results"),
        Some(&serde_json::json!([])),
        "a grid point that measured nothing is echoed as nothing"
    );
}

// A benchmark that reports zero entries is the same story: the name is born, and
// nothing else is. This is what BMF v0's `{"bench": {}}` says in v1's shape.
#[tokio::test]
async fn v1_benchmark_without_entries_mints_nothing() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "no-entries").await;

    report(&server, &fixture, 1, vec![v1("bench", &[])], None, None).await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(JsonParameters::default(), 0)],
    );
    assert_eq!(named_values(&mut conn, project_id), vec![]);
    assert_eq!(series_measures(&mut conn, project_id), Vec::<String>::new());
    assert_eq!(metric_counts(&mut conn), vec![0]);
}

// BMF v0 does not move. A v0 benchmark that reports no measures writes the
// `report_benchmark` row on its empty parameter set exactly as it always has,
// which is why the skip above is gated on the payload's version rather than
// applied to every shape that happens to be empty.
#[tokio::test]
async fn v0_benchmark_without_measures_is_unchanged() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0-no-measures").await;

    report(
        &server,
        &fixture,
        1,
        vec![serde_json::to_string(&serde_json::json!({ "bench": {} })).expect("it serializes")],
        None,
        None,
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(JsonParameters::default(), 1)],
        "the v0 row still lands on the empty parameter set"
    );
    assert_eq!(named_values(&mut conn, project_id), vec![]);
    assert_eq!(series_measures(&mut conn, project_id), Vec::<String>::new());
    assert_eq!(metric_counts(&mut conn), vec![0]);
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

    // The production meter itself, not a copy of its filter: this is the function
    // the plan check and the billing read call.
    let organization_id = organization_id(&mut conn, project_id);
    let billable = QueryMetric::usage(
        &mut conn,
        organization_id,
        base_timestamp(),
        DateTime::now(),
    )
    .expect("Failed to count the billable metrics");
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
    assert_eq!(
        u32::try_from(counted).expect("a non-negative count"),
        billable
    );
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

// Parameter sets are minted straight from report content, one row per grid point, so
// they carry the same per project creation ceiling as every other entity a report
// mints. Without it a harness that interpolates a commit sha or a timestamp into its
// parameters mints rows, grid points, and billable series without bound.
#[tokio::test]
async fn parameter_creation_is_rate_limited() {
    // Four creations per project per window. A benchmark is born with its empty
    // parameter set, which is one of the four, so the fourth new grid point under one
    // benchmark is the one that is refused.
    let server = TestServer::new_with_creation_limits(4, 4).await;

    let measures = serde_json::json!({ "latency": { "value": 1.0 } });
    let grid_points = |count: usize| -> Vec<String> {
        let entries = (0..count)
            .map(|n| entry(&serde_json::json!({ "n": n }), &measures))
            .collect::<Vec<_>>();
        vec![v1("bench", &entries)]
    };

    // Under the ceiling: three new grid points on top of the birth empty set.
    let under = fixture(&server, "under-limit").await;
    let (status, body) = try_report(&server, &under, 1, grid_points(3), None, None).await;
    assert_eq!(status, StatusCode::CREATED, "under the ceiling: {body}");

    // Over the ceiling, and in its own project, so what is counted is this project's
    // own parameter rows rather than every project's.
    let over = fixture(&server, "over-limit").await;
    let (status, body) = try_report(&server, &over, 1, grid_points(4), None, None).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "over the ceiling: {body}"
    );
    assert!(
        body.contains("Parameter"),
        "the limit that fired is the parameter one: {body}"
    );

    // Minting stopped at the ceiling: the birth empty set plus three grid points,
    // with the fourth refused rather than written.
    let project_id = get_project_id(&server, &over.project_slug);
    let mut conn = server.db_conn();
    assert_eq!(
        parameter_sets(&mut conn, project_id).len(),
        4,
        "no parameter set is minted past the ceiling"
    );
}

// A BMF v1 measure may name only percentiles and never mention `value` at all, which
// is the one shape the deprecated `metric` field cannot describe. Its named rows are
// stored like any other and its series is billed like any other, so the report that
// created them says so: the measure comes back with its named values, and the
// deprecated `metric` is simply absent.
#[tokio::test]
async fn value_less_measure_is_stored_billed_and_echoed() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "valueless").await;

    let response = report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({
                    "latency": { "value": 1.0 },
                    "throughput": { "p50": 2.0, "p99": 3.0 },
                }),
            )],
        )],
        None,
        None,
    )
    .await;

    // Stored. Ingest is best effort, and a measure that names no point estimate is a
    // legitimate payload rather than an error.
    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();
    assert_eq!(
        named_values(&mut conn, project_id),
        vec![
            (parameters("{}"), "p50".to_owned(), 2.0),
            (parameters("{}"), "p99".to_owned(), 3.0),
            (parameters("{}"), "value".to_owned(), 1.0),
        ],
    );

    // Billed. Every measure of a grid point is its own active series, the one that
    // named no point estimate included.
    assert_eq!(
        series_measures(&mut conn, project_id),
        vec!["latency".to_owned(), "throughput".to_owned()],
    );

    // Echoed. What is stored and billed is visible in the report that created it, so
    // both measures come back.
    let measures = response
        .pointer("/results/0/0/measures")
        .and_then(serde_json::Value::as_array)
        .expect("the report echoes its measures");
    let slugs: Vec<Option<&str>> = measures
        .iter()
        .map(|measure| {
            measure
                .pointer("/measure/slug")
                .and_then(serde_json::Value::as_str)
        })
        .collect();
    assert_eq!(
        slugs,
        vec![Some("latency"), Some("throughput")],
        "the measure that named no point estimate is echoed too, got {measures:#?}"
    );
    assert_eq!(
        measures[0].pointer("/metric/value"),
        Some(&serde_json::json!(1.0)),
        "the measure that named a point estimate keeps its deprecated triple"
    );

    // The deprecated `metric` is absent rather than null, because there is no `value`
    // row to reconstruct it from and nothing else may stand in for one.
    let value_less = measures[1].as_object().expect("the measure is an object");
    assert!(
        !value_less.contains_key("metric"),
        "the deprecated metric is absent, got {value_less:#?}"
    );
    let named: Vec<(Option<&str>, Option<f64>)> = value_less
        .get("metrics")
        .and_then(serde_json::Value::as_array)
        .expect("the measure echoes its named values")
        .iter()
        .map(|metric| {
            (
                metric.pointer("/name").and_then(serde_json::Value::as_str),
                metric.pointer("/value").and_then(serde_json::Value::as_f64),
            )
        })
        .collect();
    assert_eq!(
        named,
        vec![(Some("p50"), Some(2.0)), (Some("p99"), Some(3.0))],
        "the named values are the whole of what was reported"
    );
    // The other two deprecated fields are null, as they are for any ungated measure:
    // a bare threshold gates the `value` name, so this measure has no boundary.
    assert_eq!(
        value_less.get("threshold"),
        Some(&serde_json::Value::Null),
        "the deprecated threshold is null"
    );
    assert_eq!(
        value_less.get("boundary"),
        Some(&serde_json::Value::Null),
        "the deprecated boundary is null"
    );

    // Counted, because it is returned: the endpoint that loads a report's results and
    // the one that counts them without loading say the same thing about the same
    // report.
    assert_eq!(
        response.pointer("/counts/results/0"),
        Some(&serde_json::json!({ "benchmarks": 1, "measures": 2 })),
    );

    // The legacy metric-count meter is a separate question that
    // `named_values_do_not_change_the_metric_count` records rather than settles: it
    // counts `value` rows, so this report meters one. Pinned here so a change to
    // either view cannot pass unnoticed.
    let counted: i32 = schema::metric_count_by_report::table
        .select(schema::metric_count_by_report::metric_count)
        .first(&mut conn)
        .expect("Failed to read the report's metric count");
    assert_eq!(counted, 1);
}

// The compatibility claim behind an optional `metric`: nothing an older client can
// produce loses the field. BMF v0 is that shape, and its response is exactly what it
// always was, the deprecated triple included and no field turned null.
#[tokio::test]
async fn v0_measure_response_is_unchanged() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0-shape").await;

    let response = report(
        &server,
        &fixture,
        1,
        vec![
            serde_json::to_string(&serde_json::json!({
                "bench": {
                    "latency": { "value": 10.0, "lower_value": 9.0, "upper_value": 11.0 }
                }
            }))
            .expect("the results serialize"),
        ],
        None,
        None,
    )
    .await;

    let mut measure = response
        .pointer("/results/0/0/measures/0")
        .expect("the report echoes its measure")
        .clone();
    // The measure entity and every UUID are minted per run; everything else is pinned.
    *measure
        .pointer_mut("/measure")
        .expect("the measure carries its entity") = serde_json::json!("<measure>");
    *measure
        .pointer_mut("/metric/uuid")
        .expect("the deprecated metric carries a uuid") = serde_json::json!("<uuid>");
    for metric in measure["metrics"]
        .as_array_mut()
        .expect("the measure echoes its named values")
    {
        *metric
            .pointer_mut("/uuid")
            .expect("the named value carries a uuid") = serde_json::json!("<uuid>");
    }

    assert_eq!(
        measure,
        serde_json::json!({
            "measure": "<measure>",
            "metrics": [
                { "uuid": "<uuid>", "name": "lower_value", "value": 9.0, "boundaries": [] },
                { "uuid": "<uuid>", "name": "upper_value", "value": 11.0, "boundaries": [] },
                { "uuid": "<uuid>", "name": "value", "value": 10.0, "boundaries": [] },
            ],
            "metric": {
                "uuid": "<uuid>",
                "value": 10.0,
                "lower_value": 9.0,
                "upper_value": 11.0,
            },
            "threshold": null,
            "boundary": null,
        }),
    );
}

// `--fold` is deprecated, not deleted: a BMF v0 report folds exactly as it always
// has. The iterations collapse into one `report_benchmark` row on the benchmark's
// empty parameter set, carrying the folded triple, and the metric count meter that
// bills Team, Enterprise, and licences reads exactly what it read before.
//
// The whole fold path was rewritten by this layer to speak parameter sets, so this
// is the test that a pipeline running `bencher run --fold min` today sees no change
// at all.
#[tokio::test]
async fn v0_fold_still_folds() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0-fold").await;

    // One BMF v0 payload per iteration, each a whole metric triple.
    let iteration = |value: f64| {
        serde_json::to_string(&serde_json::json!({
            "bench": {
                "latency": {
                    "value": value,
                    "lower_value": value - 1.0,
                    "upper_value": value + 1.0,
                }
            }
        }))
        .expect("the results serialize")
    };

    report(
        &server,
        &fixture,
        1,
        vec![iteration(10.0), iteration(20.0)],
        None,
        Some("min"),
    )
    .await;

    let project_id = get_project_id(&server, &fixture.project_slug);
    let mut conn = server.db_conn();

    // One grid point, one row: the two iterations folded rather than landing apart.
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(parameters("{}"), 1)],
        "the folded report is one row on the empty parameter set"
    );

    assert_eq!(
        named_values(&mut conn, project_id),
        vec![
            (parameters("{}"), "lower_value".to_owned(), 9.0),
            (parameters("{}"), "upper_value".to_owned(), 11.0),
            (parameters("{}"), "value".to_owned(), 10.0),
        ],
        "the smaller iteration's whole triple survived the fold"
    );

    // The metric-count meter does not move: folding has always metered one metric
    // here, and it still does.
    let counted: i32 = schema::metric_count_by_report::table
        .select(schema::metric_count_by_report::metric_count)
        .first(&mut conn)
        .expect("Failed to read the report's metric count");
    assert_eq!(counted, 1, "one folded metric, metered once");
}
