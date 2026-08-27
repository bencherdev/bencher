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
use bencher_json::{
    BmfVersion, DateTime, JsonBenchmark, JsonBenchmarks, JsonParameter, JsonParameters, MetricName,
    ParameterSet, ParameterUuid, ProjectSlug, Slug,
};
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
    project_slug: ProjectSlug,
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
    // This file carries BMF v1 payloads that declare no version at all, and the
    // project gate refuses results that parse as v1 while the project is still at
    // version 0.
    server.set_bmf_version(&project.slug, BmfVersion::V1);
    Fixture {
        project_slug: project.slug,
        token: user.token,
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
fn parameter_sets(conn: &mut DbConnection, project_id: i32) -> Vec<(ParameterSet, i64)> {
    let parameters: Vec<(i32, ParameterSet)> = schema::parameter::table
        .inner_join(schema::benchmark::table)
        .filter(schema::benchmark::project_id.eq(project_id))
        .order(schema::parameter::id.asc())
        .select((schema::parameter::id, schema::parameter::set))
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
fn named_values(conn: &mut DbConnection, project_id: i32) -> Vec<(ParameterSet, String, f64)> {
    schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::parameter::table)
                .inner_join(schema::benchmark::table),
        )
        .filter(schema::benchmark::project_id.eq(project_id))
        .order((schema::parameter::id.asc(), schema::metric::name.asc()))
        .select((
            schema::parameter::set,
            schema::metric::name,
            schema::metric::value,
        ))
        .load::<(ParameterSet, MetricName, f64)>(&mut *conn)
        .expect("Failed to load the metric rows")
        .into_iter()
        .map(|(parameters, name, value)| (parameters, name.to_string(), value))
        .collect()
}

/// The parameter set and metric name of every alert in a project.
fn alerts(conn: &mut DbConnection, project_id: i32) -> Vec<(ParameterSet, String)> {
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
        .select((schema::parameter::set, schema::metric::name))
        .load::<(ParameterSet, MetricName)>(&mut *conn)
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

fn parameters(canonical: &str) -> ParameterSet {
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();

    // The empty set the benchmark was born with, plus one row per grid point.
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![
            (ParameterSet::default(), 0),
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
    let project_id = get_project_id(server, fixture.project_slug.as_ref());
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
    let project_id = get_project_id(server, fixture.project_slug.as_ref());
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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
        vec![(ParameterSet::default(), "value".to_owned())],
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();

    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(ParameterSet::default(), 1)],
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(ParameterSet::default(), 0)],
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(ParameterSet::default(), 0)],
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();

    assert_eq!(benchmark_count(&mut conn, project_id), 1);
    assert_eq!(
        parameter_sets(&mut conn, project_id),
        vec![(ParameterSet::default(), 1)],
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    {
        let mut conn = server.db_conn();
        let updated = diesel::update(
            schema::parameter::table
                .filter(schema::parameter::set.eq(parameters(r#"{"size_mb": 16}"#))),
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
        .filter(schema::parameter::set.eq(parameters(r#"{"size_mb": 16}"#)))
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

    let grid_points: Vec<ParameterSet> = results
        .iter()
        .map(|result| {
            serde_json::from_value(
                result
                    .pointer("/parameter/set")
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
    let project_id = get_project_id(&server, over.project_slug.as_ref());
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
    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
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

// An alert's JSON carries the boundary the metric broke through, with the values the
// detector computed rather than whatever the shapes of the query happen to line up.
//
// Both endpoints that render an alert reach the boundary from the alert's own
// `boundary_id`. The rest of the suite counts alerts and never reads one, so a
// transposed baseline and limit, or a boundary belonging to another metric, would
// pass it. This asserts the values, and asserts the two endpoints agree on them.
#[tokio::test]
async fn alert_json_carries_the_boundary_the_metric_exceeded() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "alertjson").await;

    // A tight history, then one value an order of magnitude above it.
    //
    // Every report also names a `p99`, which lands a second metric row and no
    // boundary. That is what makes this fixture able to see the join: metric
    // identifiers then outrun boundary identifiers, so an alert that reached its
    // boundary by the wrong column lands on another metric's row instead of
    // coincidentally landing on its own.
    let mut last = serde_json::Value::Null;
    for (day, value) in SMALL.into_iter().chain([SMALL_FINAL]).enumerate() {
        last = report(
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
            None,
        )
        .await;
    }

    // The report response renders its alerts through `get_report_alerts`.
    let report_alerts = last
        .get("alerts")
        .and_then(serde_json::Value::as_array)
        .expect("the report carries its alerts");
    assert_eq!(
        report_alerts.len(),
        1,
        "the regression raises exactly one alert: {report_alerts:?}"
    );
    let from_report = &report_alerts[0];

    let value = from_report
        .pointer("/metric/value")
        .and_then(serde_json::Value::as_f64)
        .expect("the alert carries its metric value");
    let baseline = from_report
        .pointer("/boundary/baseline")
        .and_then(serde_json::Value::as_f64)
        .expect("the alert carries its boundary baseline");
    let lower_limit = from_report
        .pointer("/boundary/lower_limit")
        .and_then(serde_json::Value::as_f64)
        .expect("a two sided model computes a lower limit");
    let upper_limit = from_report
        .pointer("/boundary/upper_limit")
        .and_then(serde_json::Value::as_f64)
        .expect("a two sided model computes an upper limit");

    assert!(
        (value - SMALL_FINAL).abs() < f64::EPSILON,
        "the alert is on the metric that regressed, got {value}"
    );
    // The baseline sits between the limits, and the metric broke through the upper
    // one. Transposing any two of the three breaks this.
    assert!(
        lower_limit < baseline && baseline < upper_limit,
        "the baseline sits between its limits, got {lower_limit} / {baseline} / {upper_limit}"
    );
    assert!(
        value > upper_limit,
        "the metric broke through the upper limit, got {value} against {upper_limit}"
    );
    assert!(
        baseline < SMALL_FINAL && baseline > SMALL[0],
        "the baseline is the mean of the history, not the outlier, got {baseline}"
    );
    assert_eq!(
        from_report.get("limit").and_then(serde_json::Value::as_str),
        Some("upper"),
        "the alert names the limit it broke through"
    );

    // The alert endpoint renders the same alert through `QueryAlert::into_json`.
    let alert_uuid = from_report
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the alert carries its uuid");
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts/{alert_uuid}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let from_endpoint: serde_json::Value = resp.json().await.expect("Failed to parse the alert");

    assert_eq!(
        &from_endpoint, from_report,
        "the two endpoints that render an alert render the same alert"
    );
}

/// Every alert of a project, from the alerts list endpoint.
async fn list_alerts(server: &TestServer, fixture: &Fixture) -> Vec<serde_json::Value> {
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
    assert_eq!(resp.status(), StatusCode::OK, "GET alerts");
    resp.json().await.expect("Failed to parse the alerts")
}

/// One alert of a project, from the alert detail endpoint.
async fn get_alert(server: &TestServer, fixture: &Fixture, alert: &str) -> serde_json::Value {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts/{alert}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET alert");
    resp.json().await.expect("Failed to parse the alert")
}

/// The alerts a report response embeds.
fn report_alerts(report: &serde_json::Value) -> Vec<serde_json::Value> {
    report
        .get("alerts")
        .and_then(serde_json::Value::as_array)
        .expect("the report carries its alerts")
        .clone()
}

fn keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys = value
        .as_object()
        .expect("the alert is an object")
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

/// Every key an alert carried before it named its grid point.
const ALERT_BEFORE_KEYS: [&str; 11] = [
    "uuid",
    "report",
    "iteration",
    "benchmark",
    "metric",
    "threshold",
    "boundary",
    "limit",
    "status",
    "created",
    "modified",
];

/// The keys naming the grid point and the gated scalar add.
const ALERT_ADDED_KEYS: [&str; 2] = ["parameter", "value"];

/// Ingest one benchmark's two grid points under one threshold, ending on `finals`.
/// Returns the fixture and the last report's response.
///
/// The history is [`SMALL`] and [`LARGE`], the same one [`ingest_grid`] lands, so
/// the boundary the detector computes here is the one it has always computed.
async fn ingest_grid_points(
    server: &TestServer,
    label: &str,
    finals: (f64, f64),
) -> (Fixture, serde_json::Value) {
    let fixture = fixture(server, label).await;
    let mut last = serde_json::Value::Null;
    for (day, (small, large)) in SMALL.into_iter().zip(LARGE).chain([finals]).enumerate() {
        last = report(
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
    (fixture, last)
}

// Two grid points of one benchmark under one threshold, one of which regresses. The
// alert names the grid point it fired on, on every surface that renders an alert.
//
// This is the disambiguation the field exists for: without it the two grid points
// raise alerts that read identically, because they share a benchmark, a measure, and
// a threshold.
#[tokio::test]
async fn alert_names_the_grid_point_that_regressed() {
    let server = TestServer::new().await;
    let (fixture, last) =
        ingest_grid_points(&server, "gridalert", (SMALL_FINAL, LARGE_FINAL)).await;

    let regressed = serde_json::json!({ "size_mb": 16 });

    // The report response that raised it.
    let embedded = report_alerts(&last);
    assert_eq!(
        embedded.len(),
        1,
        "only the grid point that regressed alerts: {embedded:?}"
    );
    assert_eq!(
        embedded[0].pointer("/parameter/set"),
        Some(&regressed),
        "the report's embedded alert names the grid point that regressed"
    );

    // The alerts list.
    let alerts = list_alerts(&server, &fixture).await;
    assert_eq!(alerts.len(), 1, "one alert: {alerts:?}");
    let from_list = &alerts[0];
    assert_eq!(
        from_list.pointer("/parameter/set"),
        Some(&regressed),
        "the alerts list names the grid point that regressed"
    );
    assert_eq!(
        from_list.pointer("/parameter/benchmark"),
        from_list.pointer("/benchmark/uuid"),
        "the grid point belongs to the benchmark the alert is on"
    );

    // The alert detail.
    let alert_uuid = from_list
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the alert carries its uuid");
    let from_endpoint = get_alert(&server, &fixture, alert_uuid).await;
    assert_eq!(
        &from_endpoint, from_list,
        "the alerts list and the alert endpoint render the same alert"
    );
    assert_eq!(
        &from_endpoint, &embedded[0],
        "the report response and the alert endpoint render the same alert"
    );
}

// An alert answers with everything it answered with before, unchanged, plus the one
// key naming the grid point. This is the compatibility claim stated as a fixture:
// the key set and the values are both pinned against what the previous shape gave
// this exact fixture.
#[tokio::test]
async fn alert_json_is_unchanged_but_for_the_grid_point() {
    let server = TestServer::new().await;
    let (fixture, _last) = ingest_grid_points(&server, "gridpin", (SMALL_FINAL, LARGE_FINAL)).await;

    let alerts = list_alerts(&server, &fixture).await;
    assert_eq!(alerts.len(), 1, "one alert: {alerts:?}");
    let alert = &alerts[0];

    assert_eq!(
        keys(alert),
        sorted(&[ALERT_BEFORE_KEYS.as_slice(), ALERT_ADDED_KEYS.as_slice()].concat()),
        "the key set is the old one plus the grid point and the gated scalar"
    );

    // Every key that was there before, with the value the previous shape gave it.
    assert_eq!(alert["iteration"], serde_json::json!(0));
    assert_eq!(alert["limit"], serde_json::json!("upper"));
    assert_eq!(alert["status"], serde_json::json!("active"));
    assert_eq!(alert["benchmark"]["name"], serde_json::json!("bench"));
    assert_eq!(alert["benchmark"]["slug"], serde_json::json!("bench"));
    assert_eq!(
        alert["metric"],
        serde_json::json!({
            "uuid": alert["metric"]["uuid"],
            "value": SMALL_FINAL,
            "lower_value": serde_json::Value::Null,
            "upper_value": serde_json::Value::Null,
        }),
        "the triple is the `value` row the boundary was computed for"
    );
    assert_eq!(
        alert["boundary"],
        serde_json::json!({
            "baseline": 12.0,
            "lower_limit": 6.806_397_375_694_743,
            "upper_limit": 17.193_602_624_305_257,
        }),
        "the boundary is the one the detector computed"
    );
    assert_eq!(
        alert["threshold"]["branch"]["slug"],
        serde_json::json!("main")
    );
    assert_eq!(
        alert["threshold"]["testbed"]["slug"],
        serde_json::json!("localhost")
    );
    assert_eq!(
        alert["threshold"]["measure"]["slug"],
        serde_json::json!("latency")
    );
    assert_eq!(
        alert["threshold"]["model"]["test"],
        serde_json::json!("t_test")
    );
    assert_eq!(
        alert["threshold"]["model"]["min_sample_size"],
        serde_json::json!(2)
    );
    assert_eq!(
        alert["threshold"]["model"]["max_sample_size"],
        serde_json::json!(64)
    );
    assert_eq!(
        alert["threshold"]["branch"]["head"]["version"]["number"],
        serde_json::json!(5),
        "the alert is on the version the last report landed"
    );
    assert!(
        alert["report"].is_string(),
        "the alert names the report it landed in"
    );

    // The additions: the grid point the alert fired on, and the scalar it fired on.
    assert_eq!(
        alert["parameter"]["set"],
        serde_json::json!({ "size_mb": 16 })
    );
    assert_eq!(alert["parameter"]["benchmark"], alert["benchmark"]["uuid"]);
    assert_eq!(
        alert["value"],
        serde_json::json!(SMALL_FINAL),
        "the scalar the alert fired on"
    );
    assert_eq!(
        alert["threshold"]["metric"],
        serde_json::Value::Null,
        "a bare threshold gates the conventional `value` name, so it names none"
    );
    assert_eq!(
        alert["threshold"]["parameters"],
        serde_json::Value::Null,
        "a bare threshold gates every grid point, so it names no filter"
    );
}

// The report response's embedded alerts are the alerts endpoint's alerts, for the
// same report: the same uuids, and the same grid points.
//
// The two used to be two different queries against the same view; they are now two
// different queries against the same base tables, and this is what keeps them from
// drifting apart.
#[tokio::test]
async fn report_alerts_are_the_alerts_endpoint_alerts() {
    let server = TestServer::new().await;
    // Both grid points regress on the final report, so the report carries more than
    // one alert and the two reads have to agree on a set of alerts, not just on one.
    let (fixture, last) =
        ingest_grid_points(&server, "gridboth", (SMALL_FINAL, LARGE_FINAL * 10.0)).await;

    let embedded = report_alerts(&last);
    assert_eq!(
        embedded.len(),
        2,
        "both grid points regressed: {embedded:?}"
    );

    let report_uuid = last
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the report carries its uuid");
    let mut from_endpoint = list_alerts(&server, &fixture)
        .await
        .into_iter()
        .filter(|alert| alert["report"] == serde_json::json!(report_uuid))
        .collect::<Vec<_>>();

    // Sorted by uuid, which is unique, so the order is the alerts themselves rather
    // than anything either read chose.
    let identity = |alert: &serde_json::Value| alert["uuid"].to_string();
    let mut embedded = embedded;
    embedded.sort_by_key(identity);
    from_endpoint.sort_by_key(identity);
    assert_eq!(
        embedded, from_endpoint,
        "the report's alerts are the alerts endpoint's alerts for that report"
    );

    // Two alerts, two grid points. Sorted by the set so the assertion does not ride
    // on whichever alert was written first.
    let mut sets = embedded
        .iter()
        .map(|alert| alert["parameter"]["set"].to_string())
        .collect::<Vec<_>>();
    sets.sort();
    assert_eq!(
        sets,
        vec![
            serde_json::json!({ "size_mb": 16 }).to_string(),
            serde_json::json!({ "size_mb": 32 }).to_string()
        ],
        "the two alerts name the two grid points, one each"
    );
}

/// Ingest one grid point's history, regressing at the end, with `measures` deciding
/// which named values each report carries. Returns the alert the regression raised.
async fn alert_for_bounds(
    label: &str,
    measures: impl Fn(f64) -> serde_json::Value,
) -> serde_json::Value {
    let server = TestServer::new().await;
    let fixture = fixture(&server, label).await;
    for (day, value) in SMALL.into_iter().chain([SMALL_FINAL]).enumerate() {
        report(
            &server,
            &fixture,
            day + 1,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({ "size_mb": 16 }),
                    &measures(value),
                )],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let alerts = list_alerts(&server, &fixture).await;
    assert_eq!(alerts.len(), 1, "one alert: {alerts:?}");
    let from_list = alerts.into_iter().next().expect("the only alert");
    let alert_uuid = from_list
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the alert carries its uuid")
        .to_owned();
    let from_endpoint = get_alert(&server, &fixture, &alert_uuid).await;
    assert_eq!(
        from_endpoint, from_list,
        "the alert list and the alert endpoint render the same alert"
    );
    from_list
}

// The metric triple an alert carries is assembled from the `value` row the boundary
// was computed for and that row's bound siblings, so it is the same triple for a row
// with both bounds, either bound alone, and no bounds at all.
#[tokio::test]
async fn alert_metric_triple_carries_every_bound_shape() {
    let both = alert_for_bounds("boundsboth", |value| {
        serde_json::json!({
            "latency": { "value": value, "lower_value": value * 0.9, "upper_value": value * 1.1 }
        })
    })
    .await;
    assert_eq!(
        both["metric"]["value"],
        serde_json::json!(SMALL_FINAL),
        "the triple is built around the row that alerted"
    );
    assert_eq!(both["metric"]["lower_value"], serde_json::json!(900.0));
    assert_eq!(both["metric"]["upper_value"], serde_json::json!(1_100.0));

    let lower = alert_for_bounds(
        "boundslower",
        |value| serde_json::json!({ "latency": { "value": value, "lower_value": value * 0.9 } }),
    )
    .await;
    assert_eq!(lower["metric"]["value"], serde_json::json!(SMALL_FINAL));
    assert_eq!(lower["metric"]["lower_value"], serde_json::json!(900.0));
    assert_eq!(
        lower["metric"]["upper_value"],
        serde_json::Value::Null,
        "the bound that was never reported stays absent"
    );

    let upper = alert_for_bounds(
        "boundsupper",
        |value| serde_json::json!({ "latency": { "value": value, "upper_value": value * 1.1 } }),
    )
    .await;
    assert_eq!(upper["metric"]["value"], serde_json::json!(SMALL_FINAL));
    assert_eq!(
        upper["metric"]["lower_value"],
        serde_json::Value::Null,
        "the bound that was never reported stays absent"
    );
    assert_eq!(upper["metric"]["upper_value"], serde_json::json!(1_100.0));

    let none = alert_for_bounds(
        "boundsnone",
        |value| serde_json::json!({ "latency": { "value": value } }),
    )
    .await;
    assert_eq!(none["metric"]["value"], serde_json::json!(SMALL_FINAL));
    assert_eq!(none["metric"]["lower_value"], serde_json::Value::Null);
    assert_eq!(none["metric"]["upper_value"], serde_json::Value::Null);

    // The bounds are the only thing that moved: the boundary the detector computed
    // is the same in all four, because the `value` series is the same in all four.
    for alert in [&both, &lower, &upper, &none] {
        assert_eq!(
            alert["boundary"],
            serde_json::json!({
                "baseline": 12.0,
                "lower_limit": 6.806_397_375_694_743,
                "upper_limit": 17.193_602_624_305_257,
            }),
        );
        assert_eq!(
            alert["parameter"]["set"],
            serde_json::json!({ "size_mb": 16 })
        );
    }
}

// The parameter set resource endpoints, nested under their benchmark.
//
// A parameter set has neither a name nor a slug, so every one of these routes
// addresses it by UUID, the way a report or an alert is addressed.

/// Create a benchmark through the API and return the slug the parameter routes
/// nest under.
async fn create_benchmark(server: &TestServer, fixture: &Fixture, name: &str) -> String {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/benchmarks", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "POST benchmark");
    let benchmark: JsonBenchmark = resp.json().await.expect("Failed to parse the benchmark");
    benchmark.slug.to_string()
}

/// The benchmark a report created, taken from the benchmarks endpoint so that the
/// slug under test is the one the server minted.
async fn only_benchmark(server: &TestServer, fixture: &Fixture) -> String {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/benchmarks", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET benchmarks");
    let benchmarks: JsonBenchmarks = resp.json().await.expect("Failed to parse the benchmarks");
    assert_eq!(benchmarks.0.len(), 1, "expected exactly one benchmark");
    benchmarks
        .0
        .first()
        .expect("the only benchmark")
        .slug
        .to_string()
}

async fn post_parameter(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    token: &str,
    set: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .post(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{benchmark}/parameters",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&serde_json::json!({ "set": set }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

async fn create_parameter(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    set: &serde_json::Value,
) -> JsonParameter {
    let (status, body) = post_parameter(server, fixture, benchmark, &fixture.token, set).await;
    assert_eq!(status, StatusCode::CREATED, "POST parameter: {body}");
    serde_json::from_str(&body).expect("Failed to parse the parameter")
}

/// A parameter list request, with the `X-Total-Count` header it answered with.
async fn list_parameters(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    token: &str,
    query: &str,
) -> (StatusCode, Option<String>, String) {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{benchmark}/parameters{query}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let total_count = resp
        .headers()
        .get("x-total-count")
        .and_then(|total_count| total_count.to_str().ok())
        .map(ToOwned::to_owned);
    let body = resp.text().await.expect("Failed to read the response");
    (status, total_count, body)
}

async fn parameter_list(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    query: &str,
) -> Vec<JsonParameter> {
    let (status, _, body) =
        list_parameters(server, fixture, benchmark, &fixture.token, query).await;
    assert_eq!(status, StatusCode::OK, "GET parameters: {body}");
    let parameters: JsonParameters =
        serde_json::from_str(&body).expect("Failed to parse the parameters");
    parameters.0
}

async fn get_parameter(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    token: &str,
    parameter: &ParameterUuid,
) -> (StatusCode, String) {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{benchmark}/parameters/{parameter}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

async fn patch_parameter(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    token: &str,
    parameter: &ParameterUuid,
    update: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{benchmark}/parameters/{parameter}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(update)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

async fn delete_parameter(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
    token: &str,
    parameter: &ParameterUuid,
) -> (StatusCode, String) {
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{benchmark}/parameters/{parameter}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

async fn delete_report(
    server: &TestServer,
    fixture: &Fixture,
    report: &str,
) -> (StatusCode, String) {
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/reports/{report}",
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
    (status, body)
}

/// The row id of a parameter set, or `None` once it is deleted.
fn parameter_row_id(conn: &mut DbConnection, parameter: &ParameterUuid) -> Option<i32> {
    use diesel::OptionalExtension as _;

    schema::parameter::table
        .filter(schema::parameter::uuid.eq(parameter))
        .select(schema::parameter::id)
        .first(conn)
        .optional()
        .expect("Failed to query the parameter set")
}

fn report_benchmarks_for_parameter(conn: &mut DbConnection, parameter_id: i32) -> i64 {
    schema::report_benchmark::table
        .filter(schema::report_benchmark::parameter_id.eq(parameter_id))
        .count()
        .get_result(conn)
        .expect("Failed to count the report benchmarks")
}

fn series_for_parameter(conn: &mut DbConnection, parameter_id: i32) -> i64 {
    schema::series_last_seen::table
        .filter(schema::series_last_seen::parameter_id.eq(parameter_id))
        .count()
        .get_result(conn)
        .expect("Failed to count the billable series")
}

fn project_metric_count(conn: &mut DbConnection, project_id: i32) -> i64 {
    schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::benchmark::table))
        .filter(schema::benchmark::project_id.eq(project_id))
        .count()
        .get_result(conn)
        .expect("Failed to count the metrics")
}

// A benchmark is born with exactly one parameter set, the empty one, and it is
// listed with its total count like every other dimension list.
#[tokio::test]
async fn parameter_list_starts_with_the_empty_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "list-empty").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;

    let (status, total_count, body) =
        list_parameters(&server, &fixture, &benchmark, &fixture.token, "").await;
    assert_eq!(status, StatusCode::OK, "GET parameters: {body}");
    assert_eq!(total_count.as_deref(), Some("1"));

    let parameters: JsonParameters =
        serde_json::from_str(&body).expect("Failed to parse the parameters");
    assert_eq!(
        parameters.0.len(),
        1,
        "a benchmark is born with exactly one parameter set: {body}"
    );
    let parameter = parameters.0.first().expect("the only parameter set");
    assert_eq!(parameter.set, ParameterSet::default());
    assert!(
        parameter.archived.is_none(),
        "the birth parameter set is not archived"
    );
}

// The list is sorted by creation, oldest first, and pages the way every other
// dimension list pages.
#[tokio::test]
async fn parameter_list_paginates_in_creation_order() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "list-page").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;

    let mut created = Vec::new();
    for size_mb in [16, 32, 64] {
        let parameter = create_parameter(
            &server,
            &fixture,
            &benchmark,
            &serde_json::json!({ "size_mb": size_mb }),
        )
        .await;
        created.push(parameter.uuid);
    }

    let (status, total_count, body) =
        list_parameters(&server, &fixture, &benchmark, &fixture.token, "").await;
    assert_eq!(status, StatusCode::OK, "GET parameters: {body}");
    assert_eq!(total_count.as_deref(), Some("4"));

    // The empty set the benchmark was born with, then the three grid points in the
    // order they were created.
    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 4);
    assert_eq!(
        all.first().expect("the first parameter set").set,
        ParameterSet::default(),
        "the empty set the benchmark was born with sorts first"
    );
    assert_eq!(
        all.iter().skip(1).map(|p| p.uuid).collect::<Vec<_>>(),
        created,
        "the grid points list in creation order"
    );

    let first_page = parameter_list(&server, &fixture, &benchmark, "?per_page=2&page=1").await;
    let second_page = parameter_list(&server, &fixture, &benchmark, "?per_page=2&page=2").await;
    assert_eq!(
        first_page
            .iter()
            .chain(second_page.iter())
            .map(|p| p.uuid)
            .collect::<Vec<_>>(),
        all.iter().map(|p| p.uuid).collect::<Vec<_>>(),
        "the two pages are the whole list, in order"
    );

    let descending = parameter_list(&server, &fixture, &benchmark, "?direction=desc").await;
    assert_eq!(
        descending.iter().map(|p| p.uuid).collect::<Vec<_>>(),
        all.iter().rev().map(|p| p.uuid).collect::<Vec<_>>(),
        "descending is the same list, reversed"
    );
}

// The archived filter has the same two states a benchmark's does: archived sets are
// out of the default list and are the only thing `archived=true` returns.
#[tokio::test]
async fn parameter_list_filters_by_archived() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "list-archived").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let parameter = create_parameter(
        &server,
        &fixture,
        &benchmark,
        &serde_json::json!({ "size_mb": 16 }),
    )
    .await;

    let (status, body) = patch_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &parameter.uuid,
        &serde_json::json!({ "archived": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PATCH parameter: {body}");

    let default = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(default.len(), 1, "only the empty set is left unarchived");
    assert_eq!(
        default.first().expect("the empty set").set,
        ParameterSet::default(),
        "the archived set is out of the default list"
    );

    let archived = parameter_list(&server, &fixture, &benchmark, "?archived=true").await;
    assert_eq!(archived.len(), 1);
    let archived = archived.first().expect("the archived set");
    assert_eq!(archived.uuid, parameter.uuid);
    assert!(
        archived.archived.is_some(),
        "the archived set carries its timestamp"
    );
}

// A created parameter set reads back the same through the one get endpoint.
#[tokio::test]
async fn parameter_get_reads_back_the_created_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "get-one").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let created = create_parameter(
        &server,
        &fixture,
        &benchmark,
        &serde_json::json!({ "op": "read", "size_mb": 16 }),
    )
    .await;

    let (status, body) =
        get_parameter(&server, &fixture, &benchmark, &fixture.token, &created.uuid).await;
    assert_eq!(status, StatusCode::OK, "GET parameter: {body}");
    let parameter: JsonParameter = serde_json::from_str(&body).expect("Failed to parse");
    assert_eq!(parameter.uuid, created.uuid);
    assert_eq!(parameter.set, parameters(r#"{"op":"read","size_mb":16}"#));
    assert_eq!(parameter.benchmark, created.benchmark);
}

// A parameter set that already exists under the benchmark is a conflict. This
// endpoint is create, not get-or-create: the empty set the benchmark was born with
// conflicts the same way any other repeated set does.
#[tokio::test]
async fn parameter_post_duplicate_is_a_conflict() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "post-duplicate").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;

    let set = serde_json::json!({ "size_mb": 16 });
    let created = create_parameter(&server, &fixture, &benchmark, &set).await;

    let (status, body) = post_parameter(&server, &fixture, &benchmark, &fixture.token, &set).await;
    assert_eq!(status, StatusCode::CONFLICT, "POST duplicate: {body}");

    // A set that is logically the same set, spelled differently, is the same set.
    let (status, body) = post_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &serde_json::json!({ "size_mb": 16.0 }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "POST respelled: {body}");

    let (status, body) = post_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "POST the empty set: {body}");

    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(
        all.len(),
        2,
        "the birth empty set and the one created grid point"
    );
    assert!(all.iter().any(|parameter| parameter.uuid == created.uuid));
}

// The endpoint mints parameter sets, so it carries the same per project ceiling the
// report path does, with the same error.
#[tokio::test]
async fn parameter_post_is_rate_limited() {
    // Four creations per project per window. A benchmark is born with its empty
    // parameter set, which is one of the four, so the fourth posted grid point is
    // the one that is refused.
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let fixture = fixture(&server, "post-limit").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;

    for size_mb in [16, 32, 64] {
        create_parameter(
            &server,
            &fixture,
            &benchmark,
            &serde_json::json!({ "size_mb": size_mb }),
        )
        .await;
    }

    let (status, body) = post_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &serde_json::json!({ "size_mb": 128 }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "over the ceiling: {body}"
    );
    assert!(
        body.contains("Parameter"),
        "the limit that fired is the parameter one: {body}"
    );

    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 4, "no parameter set is minted past the ceiling");
}

// Archiving sets the timestamp and unarchiving clears it, exactly as it does for a
// benchmark.
#[tokio::test]
async fn parameter_patch_archives_and_unarchives() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "patch-archive").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let created = create_parameter(
        &server,
        &fixture,
        &benchmark,
        &serde_json::json!({ "size_mb": 16 }),
    )
    .await;
    assert!(created.archived.is_none());

    let (status, body) = patch_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &created.uuid,
        &serde_json::json!({ "archived": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PATCH archive: {body}");
    let archived: JsonParameter = serde_json::from_str(&body).expect("Failed to parse");
    assert!(archived.archived.is_some(), "archiving sets the timestamp");
    assert_eq!(archived.set, created.set, "archiving does not move the set");

    let (status, body) = patch_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &created.uuid,
        &serde_json::json!({ "archived": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PATCH unarchive: {body}");
    let unarchived: JsonParameter = serde_json::from_str(&body).expect("Failed to parse");
    assert!(
        unarchived.archived.is_none(),
        "unarchiving clears the timestamp"
    );
}

// Archiving the empty parameter set is allowed, because a later report revives it.
#[tokio::test]
async fn parameter_patch_archives_the_empty_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "patch-empty").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    let empty_set = all.first().expect("the empty parameter set").uuid;

    let (status, body) = patch_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &empty_set,
        &serde_json::json!({ "archived": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PATCH the empty set: {body}");

    let archived = parameter_list(&server, &fixture, &benchmark, "?archived=true").await;
    assert_eq!(archived.len(), 1);
    assert_eq!(
        archived.first().expect("the archived empty set").uuid,
        empty_set
    );
}

// A parameter set that a report still references cannot be deleted. Its results
// have to be deleted first, exactly as a benchmark's do one level up, so the
// deletable state is a parameter set nothing points at any more.
#[tokio::test]
async fn parameter_delete_refuses_while_reports_reference_it() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-referenced").await;

    let measures = serde_json::json!({ "latency": { "value": 1.0 } });
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[
                entry(&serde_json::json!({}), &measures),
                entry(&serde_json::json!({ "size_mb": 16 }), &measures),
            ],
        )],
        None,
        None,
    )
    .await;

    let benchmark = only_benchmark(&server, &fixture).await;
    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 2);
    let grid_point = all
        .iter()
        .find(|parameter| parameter.set == parameters(r#"{"size_mb":16}"#))
        .expect("the reported grid point");

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    let grid_point_id = parameter_row_id(&mut conn, &grid_point.uuid).expect("the grid point row");
    assert_eq!(report_benchmarks_for_parameter(&mut conn, grid_point_id), 1);
    drop(conn);

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a referenced parameter set is not deletable: {body}"
    );

    // Nothing moved: the parameter set, its results, its metrics, and its billable
    // series are all still there.
    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_some(),
        "the parameter set is still there"
    );
    assert_eq!(report_benchmarks_for_parameter(&mut conn, grid_point_id), 1);
    assert_eq!(series_for_parameter(&mut conn, grid_point_id), 1);
    assert_eq!(project_metric_count(&mut conn, project_id), 2);
    drop(conn);

    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 2, "both parameter sets are still listed");
}

// A parameter set nothing references any more is deletable, and the billable series
// it left behind go with it. Deleting the report is what unreferences it: the report
// takes its own results, and the parameter set and its series outlive them.
#[tokio::test]
async fn parameter_delete_removes_an_unreferenced_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-unreferenced").await;

    let measures = serde_json::json!({ "latency": { "value": 1.0 } });
    let json_report = report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[
                entry(&serde_json::json!({}), &measures),
                entry(&serde_json::json!({ "size_mb": 16 }), &measures),
            ],
        )],
        None,
        None,
    )
    .await;
    let report_uuid = json_report
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the report carries its uuid")
        .to_owned();

    let benchmark = only_benchmark(&server, &fixture).await;
    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    let grid_point = all
        .iter()
        .find(|parameter| parameter.set == parameters(r#"{"size_mb":16}"#))
        .expect("the reported grid point");
    let empty_set = all
        .iter()
        .find(|parameter| parameter.set == ParameterSet::default())
        .expect("the empty parameter set");

    let mut conn = server.db_conn();
    let grid_point_id = parameter_row_id(&mut conn, &grid_point.uuid).expect("the grid point row");
    let empty_set_id = parameter_row_id(&mut conn, &empty_set.uuid).expect("the empty set row");
    drop(conn);

    let (status, body) = delete_report(&server, &fixture, &report_uuid).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "DELETE report: {body}");

    // The report took its results, and left the parameter sets and the series they
    // billed behind.
    let mut conn = server.db_conn();
    assert_eq!(
        report_benchmarks_for_parameter(&mut conn, grid_point_id),
        0,
        "the report took its results"
    );
    assert_eq!(
        series_for_parameter(&mut conn, grid_point_id),
        1,
        "the billable series outlives the report"
    );
    drop(conn);

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "DELETE parameter: {body}");

    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_none(),
        "the parameter set is gone"
    );
    assert_eq!(
        series_for_parameter(&mut conn, grid_point_id),
        0,
        "its billable series go with it"
    );
    // The benchmark's empty parameter set is untouched.
    assert!(
        parameter_row_id(&mut conn, &empty_set.uuid).is_some(),
        "the empty set is still there"
    );
    assert_eq!(series_for_parameter(&mut conn, empty_set_id), 1);
    drop(conn);

    let remaining = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining.first().expect("the empty set").uuid,
        empty_set.uuid
    );
}

// The empty parameter set is structural, so this endpoint may not delete it. Every
// benchmark is born with exactly one, and ingest treats a missing empty set as data
// corruption rather than a set to mint.
#[tokio::test]
async fn parameter_delete_refuses_the_empty_set() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-empty").await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    let empty_set = all.first().expect("the empty parameter set").uuid;

    let (status, body) =
        delete_parameter(&server, &fixture, &benchmark, &fixture.token, &empty_set).await;
    assert_eq!(status, StatusCode::CONFLICT, "DELETE the empty set: {body}");

    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 1, "the empty set is still there");
    assert_eq!(all.first().expect("the empty set").uuid, empty_set);
}

// A parameter set under one benchmark is not addressable through another, and a
// UUID that does not exist is not found.
#[tokio::test]
async fn parameter_get_is_scoped_to_its_benchmark() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "get-scope").await;
    let first = create_benchmark(&server, &fixture, "bench one").await;
    let second = create_benchmark(&server, &fixture, "bench two").await;
    let parameter = create_parameter(
        &server,
        &fixture,
        &first,
        &serde_json::json!({ "size_mb": 16 }),
    )
    .await;

    let (status, _) =
        get_parameter(&server, &fixture, &second, &fixture.token, &parameter.uuid).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = get_parameter(
        &server,
        &fixture,
        &first,
        &fixture.token,
        &ParameterUuid::new(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Permissions are the benchmark endpoints' permissions: a public project's sets are
// readable by anyone, and writing one takes a role on the project.
#[tokio::test]
async fn parameter_writes_require_permission() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "permissions").await;
    let outsider = server
        .signup("Outsider", "paramsoutsider@example.com")
        .await;
    let benchmark = create_benchmark(&server, &fixture, "bench one").await;
    let parameter = create_parameter(
        &server,
        &fixture,
        &benchmark,
        &serde_json::json!({ "size_mb": 16 }),
    )
    .await;

    // The project is public, so the outsider may read.
    let (status, _, body) =
        list_parameters(&server, &fixture, &benchmark, &outsider.token, "").await;
    assert_eq!(status, StatusCode::OK, "GET parameters: {body}");
    let (status, body) = get_parameter(
        &server,
        &fixture,
        &benchmark,
        &outsider.token,
        &parameter.uuid,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET parameter: {body}");

    // Every write is refused.
    let (status, body) = post_parameter(
        &server,
        &fixture,
        &benchmark,
        &outsider.token,
        &serde_json::json!({ "size_mb": 32 }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "POST parameter: {body}");

    let (status, body) = patch_parameter(
        &server,
        &fixture,
        &benchmark,
        &outsider.token,
        &parameter.uuid,
        &serde_json::json!({ "archived": true }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "PATCH parameter: {body}");

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &outsider.token,
        &parameter.uuid,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "DELETE parameter: {body}");

    let all = parameter_list(&server, &fixture, &benchmark, "").await;
    assert_eq!(all.len(), 2, "nothing the outsider sent landed");
    assert!(all.iter().all(|parameter| parameter.archived.is_none()));
}

/// Create a threshold through the thresholds endpoint.
///
/// `metric` names the metric it gates and `parameters` is the filter over grid
/// points; either omitted is the default that half of the identity carries.
async fn post_threshold(
    server: &TestServer,
    fixture: &Fixture,
    metric: Option<&str>,
    parameters: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "measure": "latency",
        "metric": metric,
        "parameters": parameters,
        "test": "t_test",
        "min_sample_size": 2,
        "max_sample_size": 64,
        "lower_boundary": 0.98,
        "upper_boundary": 0.98,
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", fixture.project_slug)))
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

/// Create a threshold and return it, failing the test if the server refused.
async fn create_threshold(
    server: &TestServer,
    fixture: &Fixture,
    metric: Option<&str>,
    parameters: Option<serde_json::Value>,
) -> serde_json::Value {
    let (status, body) = post_threshold(server, fixture, metric, parameters).await;
    assert_eq!(status, StatusCode::CREATED, "POST threshold: {body}");
    serde_json::from_str(&body).expect("Failed to parse the threshold")
}

/// The UUID of one of a project's resources, addressed by slug.
///
/// The perf endpoint takes UUIDs and nothing else, so every dimension a perf query
/// names goes through here first.
async fn resource_uuid(
    server: &TestServer,
    fixture: &Fixture,
    resource: &str,
    slug: &str,
) -> String {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/{resource}/{slug}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET {resource}/{slug}");
    let json: serde_json::Value = resp.json().await.expect("Failed to parse the resource");
    json.get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the resource names its uuid")
        .to_owned()
}

/// The `latency` lines of one benchmark, from the perf endpoint.
async fn perf_line(
    server: &TestServer,
    fixture: &Fixture,
    benchmark: &str,
) -> Vec<serde_json::Value> {
    let branch = resource_uuid(server, fixture, "branches", "main").await;
    let testbed = resource_uuid(server, fixture, "testbeds", "localhost").await;
    let measure = resource_uuid(server, fixture, "measures", "latency").await;
    let benchmark = resource_uuid(server, fixture, "benchmarks", benchmark).await;
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/perf?branches={branch}&testbeds={testbed}&benchmarks={benchmark}&measures={measure}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET perf");
    let perf: serde_json::Value = resp.json().await.expect("Failed to parse the perf");
    perf.get("results")
        .and_then(serde_json::Value::as_array)
        .expect("the perf results are a list")
        .clone()
}

/// The stable `value` history the named gating fixtures report beside the name
/// under test, an order of magnitude away from it.
///
/// The gap is the point: pool the two names into one sample and its standard
/// deviation swallows the regression, so the named fixtures raise no alert at all.
const NAMED_VALUE: [f64; 6] = [1_000.0, 1_001.0, 1_002.0, 1_003.0, 1_004.0, 1_005.0];
const NAMED_P99: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const NAMED_P99_FINAL: f64 = 50.0;

// A threshold that names `p99` gates the `p99` series: it alerts on a `p99`
// regression, and the sample it tests against is `p99` rows alone.
#[tokio::test]
async fn named_threshold_gates_its_own_name() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "namedgate").await;

    // The first report mints the branch, the testbed, and the measure the threshold
    // hangs off, and carries no threshold model of its own.
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({
                    "latency": { "value": NAMED_VALUE[0], "p99": NAMED_P99[0] }
                }),
            )],
        )],
        None,
        None,
    )
    .await;

    let threshold = create_threshold(&server, &fixture, Some("p99"), None).await;
    assert_eq!(threshold["metric"], serde_json::json!("p99"));
    assert_eq!(threshold["parameters"], serde_json::Value::Null);

    for (day, (value, p99)) in NAMED_VALUE
        .into_iter()
        .skip(1)
        .zip(NAMED_P99.into_iter().skip(1).chain([NAMED_P99_FINAL]))
        .enumerate()
    {
        report(
            &server,
            &fixture,
            day + 2,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({ "size": 512 }),
                    &serde_json::json!({ "latency": { "value": value, "p99": p99 } }),
                )],
            )],
            None,
            None,
        )
        .await;
    }

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    let names = boundary_names(&mut conn, project_id);
    assert!(!names.is_empty(), "the fixture computes boundaries");
    assert!(
        names.iter().all(|name| name == "p99"),
        "a `p99` threshold gates the `p99` rows and nothing else, got {names:?}"
    );
    assert_eq!(
        alerts(&mut conn, project_id),
        vec![(parameters(r#"{"size": 512}"#), "p99".to_owned())],
        "the `p99` regression alerts, tested against the `p99` history alone"
    );
    drop(conn);

    let listed = list_alerts(&server, &fixture).await;
    assert_eq!(listed.len(), 1, "one alert: {listed:?}");
    let alert = &listed[0];
    assert_eq!(
        alert["value"],
        serde_json::json!(NAMED_P99_FINAL),
        "the alert carries the scalar it fired on"
    );
    assert_eq!(
        alert["metric"],
        serde_json::Value::Null,
        "the triple is a convention over `value`, so a `p99` alert has none"
    );
    assert_eq!(
        alert["threshold"]["metric"],
        serde_json::json!("p99"),
        "the gated name is the threshold's"
    );
    assert_eq!(
        alert["parameter"]["set"],
        serde_json::json!({ "size": 512 }),
        "the alert names the grid point it fired on"
    );
}

/// A filtered fixture's history, tight enough that the final value is an outlier
/// against it under every grid point.
const FILTERED: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const FILTERED_FINAL: f64 = 50.0;

/// Report one day of every named grid point of `bench`, each measuring `value`.
async fn report_grid(
    server: &TestServer,
    fixture: &Fixture,
    day: usize,
    sizes: &[i64],
    value: f64,
) {
    let entries = sizes
        .iter()
        .map(|size| {
            entry(
                &serde_json::json!({ "size": size }),
                &serde_json::json!({ "latency": { "value": value } }),
            )
        })
        .collect::<Vec<_>>();
    report(
        server,
        fixture,
        day,
        vec![v1("bench", &entries)],
        None,
        None,
    )
    .await;
}

// A parameters filter gates the grid points it matches and no others, and a filter
// naming several sets gates their union.
#[tokio::test]
async fn filtered_threshold_gates_the_matching_grid_points() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "filtered").await;

    let sizes = [512, 1_024, 2_048];
    report_grid(&server, &fixture, 1, &sizes, FILTERED[0]).await;
    let threshold = create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size": 512 }, { "size": 1024 }])),
    )
    .await;
    assert_eq!(
        threshold["parameters"],
        serde_json::json!([{ "size": 1024 }, { "size": 512 }]),
        "the response carries the canonical form, sorted by canonical bytes"
    );

    for (day, value) in FILTERED
        .into_iter()
        .skip(1)
        .chain([FILTERED_FINAL])
        .enumerate()
    {
        report_grid(&server, &fixture, day + 2, &sizes, value).await;
    }

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    let mut alerted = alerts(&mut conn, project_id);
    alerted.sort_by_key(|(set, _)| set.canonical());
    assert_eq!(
        alerted,
        vec![
            (parameters(r#"{"size": 1024}"#), "value".to_owned()),
            (parameters(r#"{"size": 512}"#), "value".to_owned()),
        ],
        "the filter is an OR across its sets, and the third grid point is in neither"
    );
}

// A grid point that a bare threshold and a filtered threshold both match earns a
// boundary from each and, on a regression, an alert from each. There is no winner.
#[tokio::test]
async fn every_matching_threshold_fires() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "double").await;

    // The bare threshold comes from the report's own models map, so it is the older
    // of the two and sorts first in every boundaries list.
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({ "latency": { "value": FILTERED[0] } }),
            )],
        )],
        Some(threshold_models()),
        None,
    )
    .await;
    let filtered = create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size": 512 }])),
    )
    .await;

    let mut last = serde_json::Value::Null;
    for (day, value) in FILTERED
        .into_iter()
        .skip(1)
        .chain([FILTERED_FINAL])
        .enumerate()
    {
        last = report(
            &server,
            &fixture,
            day + 2,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({ "size": 512 }),
                    &serde_json::json!({ "latency": { "value": value } }),
                )],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        alerts(&mut conn, project_id),
        vec![
            (parameters(r#"{"size": 512}"#), "value".to_owned()),
            (parameters(r#"{"size": 512}"#), "value".to_owned()),
        ],
        "one regression, two thresholds, two alerts"
    );
    drop(conn);

    // The report response lists both boundaries on the one `value` row, oldest
    // threshold first, and its deprecated singular fields carry the bare one.
    let measure = &last["results"][0][0]["measures"][0];
    let boundaries = measure["metrics"][0]["boundaries"]
        .as_array()
        .expect("the metric lists its boundaries");
    assert_eq!(boundaries.len(), 2, "two thresholds gated the row");
    assert_ne!(
        boundaries[0]["threshold"]["uuid"], filtered["uuid"],
        "the bare threshold was created first, so it is listed first"
    );
    assert_eq!(boundaries[1]["threshold"]["uuid"], filtered["uuid"]);
    assert_eq!(
        measure["threshold"]["uuid"], boundaries[0]["threshold"]["uuid"],
        "the deprecated singular threshold is the bare one"
    );
    assert_eq!(
        measure["boundary"], boundaries[0]["boundary"],
        "the deprecated singular boundary is the bare one's"
    );

    // The perf response says the same thing about the same row.
    let benchmark = only_benchmark(&server, &fixture).await;
    let line = perf_line(&server, &fixture, &benchmark).await;
    assert_eq!(line.len(), 1, "one grid point, one line");
    let point = line[0]["metrics"]
        .as_array()
        .expect("the line has points")
        .last()
        .expect("the line has a last point")
        .clone();
    let boundaries = point["metrics"]["value"]["boundaries"]
        .as_array()
        .expect("the `value` row lists its boundaries");
    assert_eq!(boundaries.len(), 2, "two thresholds gated the row");
    assert_eq!(
        point["threshold"]["uuid"], boundaries[0]["threshold"]["uuid"],
        "the deprecated singular threshold is the bare one"
    );
    assert_eq!(point["boundary"], boundaries[0]["boundary"]);
    assert_eq!(
        point["alert"]["uuid"], boundaries[0]["alert"]["uuid"],
        "the deprecated singular alert is the bare threshold's"
    );
}

// A row that only a named threshold gates reports no deprecated singular gate at
// all, which is exactly what a caller from before named gating would have seen.
#[tokio::test]
async fn legacy_fields_are_absent_without_a_bare_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "legacyabsent").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({
                    "latency": { "value": NAMED_VALUE[0], "p99": NAMED_P99[0] }
                }),
            )],
        )],
        None,
        None,
    )
    .await;
    create_threshold(&server, &fixture, Some("p99"), None).await;

    let mut last = serde_json::Value::Null;
    for (day, (value, p99)) in NAMED_VALUE
        .into_iter()
        .skip(1)
        .zip(NAMED_P99.into_iter().skip(1).chain([NAMED_P99_FINAL]))
        .enumerate()
    {
        last = report(
            &server,
            &fixture,
            day + 2,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({}),
                    &serde_json::json!({ "latency": { "value": value, "p99": p99 } }),
                )],
            )],
            None,
            None,
        )
        .await;
    }

    let measure = &last["results"][0][0]["measures"][0];
    assert_eq!(
        measure["threshold"],
        serde_json::Value::Null,
        "no bare threshold gated the `value` row, so the deprecated field is absent"
    );
    assert_eq!(measure["boundary"], serde_json::Value::Null);
    assert!(
        measure["metric"]["value"].is_number(),
        "the deprecated triple is still the `value` row's"
    );

    let benchmark = only_benchmark(&server, &fixture).await;
    let line = perf_line(&server, &fixture, &benchmark).await;
    let point = line[0]["metrics"]
        .as_array()
        .expect("the line has points")
        .last()
        .expect("the line has a last point")
        .clone();
    assert_eq!(point["threshold"], serde_json::Value::Null);
    assert_eq!(point["boundary"], serde_json::Value::Null);
    assert_eq!(point["alert"], serde_json::Value::Null);
    assert_eq!(
        point["metrics"]["p99"]["boundaries"]
            .as_array()
            .expect("the `p99` row lists its boundary")
            .len(),
        1,
        "the `p99` row carries the gate the `value` row does not"
    );
}

// Two spellings of one identity are one threshold: the second create collides.
#[tokio::test]
async fn duplicate_identity_is_refused_at_both_spellings() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "identity").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({ "latency": { "value": 1.0 } }),
            )],
        )],
        None,
        None,
    )
    .await;

    // A bare threshold, then the same one with everything it defaults to spelled out.
    create_threshold(&server, &fixture, None, None).await;
    for spelling in [
        (None, Some(serde_json::json!([]))),
        (Some("value"), None),
        (Some("value"), Some(serde_json::json!([{}]))),
    ] {
        let (status, body) = post_threshold(&server, &fixture, spelling.0, spelling.1).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "the bare threshold already exists: {body}"
        );
    }

    // A filtered threshold, then the same filter spelled another way.
    create_threshold(
        &server,
        &fixture,
        Some("p99"),
        Some(serde_json::json!([{ "size": 512 }])),
    )
    .await;
    let (status, body) = post_threshold(
        &server,
        &fixture,
        Some("p99"),
        Some(serde_json::json!([{ "size": 512.0 }, { "size": 512 }])),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "one number has one canonical spelling: {body}"
    );

    // But a filter that names a different grid point is a different threshold.
    create_threshold(
        &server,
        &fixture,
        Some("p99"),
        Some(serde_json::json!([{ "size": 1024 }])),
    )
    .await;
}

/// One metric row from the metrics endpoint, addressed by UUID.
async fn metric_row(
    server: &TestServer,
    fixture: &Fixture,
    metric_uuid: &str,
) -> serde_json::Value {
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
    assert_eq!(resp.status(), StatusCode::OK, "GET metrics/{metric_uuid}");
    resp.json().await.expect("Failed to parse the metric row")
}

// The metrics endpoint's deprecated singular gate is the bare threshold's, under a
// grid point that a bare threshold and a filtered one both gate.
//
// This is the one surface that picks the bare gate in SQL rather than in Rust, so it
// is the one that could lose the bare threshold silently. The filtered threshold is
// created first on purpose: it takes the lower identifier, so it is the row the
// outer join reaches first and the row a query with no order would keep.
#[tokio::test]
async fn metric_row_singular_gate_is_the_bare_one() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "onemetricbare").await;

    // The first report mints the branch, the testbed, and the measure the filtered
    // threshold hangs off, and carries no threshold model of its own.
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({ "latency": { "value": FILTERED[0] } }),
            )],
        )],
        None,
        None,
    )
    .await;
    let filtered = create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size": 512 }])),
    )
    .await;

    // The bare threshold comes second, from the report's own models map.
    let mut last = serde_json::Value::Null;
    for (day, value) in FILTERED
        .into_iter()
        .skip(1)
        .chain([FILTERED_FINAL])
        .enumerate()
    {
        last = report(
            &server,
            &fixture,
            day + 2,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({ "size": 512 }),
                    &serde_json::json!({ "latency": { "value": value } }),
                )],
            )],
            Some(threshold_models()),
            None,
        )
        .await;
    }

    let bare = threshold_with(
        &list_thresholds(&server, &fixture, None).await,
        &serde_json::Value::Null,
    );
    assert_ne!(bare["uuid"], filtered["uuid"], "two distinct thresholds");

    let measure = &last["results"][0][0]["measures"][0];
    let metric_uuid = measure["metrics"][0]["uuid"]
        .as_str()
        .expect("the metric names its uuid")
        .to_owned();
    let boundaries = measure["metrics"][0]["boundaries"]
        .as_array()
        .expect("the metric lists its boundaries")
        .clone();
    assert_eq!(boundaries.len(), 2, "two thresholds gated the row");
    let bare_boundary = boundaries
        .iter()
        .find(|gate| gate["threshold"]["uuid"] == bare["uuid"])
        .expect("the bare threshold gated the row");

    let row = metric_row(&server, &fixture, &metric_uuid).await;
    assert_eq!(
        row["threshold"]["uuid"], bare["uuid"],
        "the deprecated singular threshold is the bare one"
    );
    assert_ne!(
        row["threshold"]["uuid"], filtered["uuid"],
        "and never the filtered one, whichever the join reaches first"
    );
    assert_eq!(
        row["boundary"], bare_boundary["boundary"],
        "the deprecated singular boundary is the bare one's"
    );
    assert!(
        row["alert"]["uuid"].is_string(),
        "the deprecated singular alert is the bare threshold's"
    );
}

// A row that only a named threshold gates reports no singular gate on the metrics
// endpoint either, and neither does the `value` row beside it.
#[tokio::test]
async fn metric_row_singular_gate_is_absent_without_a_bare_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "onemetricnobare").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({}),
                &serde_json::json!({
                    "latency": { "value": NAMED_VALUE[0], "p99": NAMED_P99[0] }
                }),
            )],
        )],
        None,
        None,
    )
    .await;
    create_threshold(&server, &fixture, Some("p99"), None).await;

    let mut last = serde_json::Value::Null;
    for (day, (value, p99)) in NAMED_VALUE
        .into_iter()
        .skip(1)
        .zip(NAMED_P99.into_iter().skip(1).chain([NAMED_P99_FINAL]))
        .enumerate()
    {
        last = report(
            &server,
            &fixture,
            day + 2,
            vec![v1(
                "bench",
                &[entry(
                    &serde_json::json!({}),
                    &serde_json::json!({ "latency": { "value": value, "p99": p99 } }),
                )],
            )],
            None,
            None,
        )
        .await;
    }

    let measure = &last["results"][0][0]["measures"][0];
    for metric in measure["metrics"]
        .as_array()
        .expect("the measure lists its metrics")
    {
        let uuid = metric["uuid"].as_str().expect("the metric names its uuid");
        let row = metric_row(&server, &fixture, uuid).await;
        assert_eq!(
            row["threshold"],
            serde_json::Value::Null,
            "no bare threshold gated {name}",
            name = metric["name"]
        );
        assert_eq!(row["boundary"], serde_json::Value::Null);
        assert_eq!(row["alert"], serde_json::Value::Null);
    }
}

/// Every threshold of a project, from the thresholds endpoint, optionally narrowed
/// to one branch by name.
async fn list_thresholds(
    server: &TestServer,
    fixture: &Fixture,
    branch: Option<&str>,
) -> Vec<serde_json::Value> {
    let query = branch.map_or_else(String::new, |branch| format!("?branch={branch}"));
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds{query}",
            fixture.project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET thresholds");
    let thresholds: serde_json::Value = resp.json().await.expect("Failed to parse the thresholds");
    thresholds
        .as_array()
        .expect("the thresholds are a list")
        .clone()
}

/// The one threshold of a list whose parameters filter is `parameters`.
fn threshold_with(
    thresholds: &[serde_json::Value],
    parameters: &serde_json::Value,
) -> serde_json::Value {
    let matched = thresholds
        .iter()
        .filter(|threshold| {
            threshold
                .get("parameters")
                .unwrap_or(&serde_json::Value::Null)
                == parameters
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matched.len(),
        1,
        "expected one threshold filtered on {parameters}, got {matched:?}"
    );
    matched
        .first()
        .map(|threshold| (*threshold).clone())
        .expect("the matched threshold")
}

// A report's `thresholds` map names a measure and a model and nothing else, so the
// threshold it addresses is the bare one. `reset` takes a model away from the bare
// thresholds it did not name, and from no others: a threshold that gates a corner of
// the grid is addressed through the thresholds endpoint, so a report cannot reset it.
#[tokio::test]
async fn reset_leaves_a_filtered_threshold_alone() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "reset").await;

    // The report's map mints the bare threshold with a model.
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({ "latency": { "value": FILTERED[0] } }),
            )],
        )],
        Some(threshold_models()),
        None,
    )
    .await;
    create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size": 512 }])),
    )
    .await;

    let before = list_thresholds(&server, &fixture, None).await;
    assert_eq!(before.len(), 2, "one bare threshold and one filtered");
    assert!(
        threshold_with(&before, &serde_json::Value::Null)["model"]["test"].is_string(),
        "the bare threshold starts with a model"
    );

    // A reset that names nothing at all.
    report(
        &server,
        &fixture,
        2,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({ "latency": { "value": FILTERED[1] } }),
            )],
        )],
        Some(serde_json::json!({ "reset": true })),
        None,
    )
    .await;

    let after = list_thresholds(&server, &fixture, None).await;
    assert_eq!(after.len(), 2, "reset removes models, never thresholds");
    assert_eq!(
        threshold_with(&after, &serde_json::Value::Null)["model"],
        serde_json::Value::Null,
        "the reset took the bare threshold's model, because the map did not name it"
    );
    let filtered = threshold_with(&after, &serde_json::json!([{ "size": 512 }]));
    assert!(
        filtered["model"]["test"].is_string(),
        "the filtered threshold keeps its model: no report map addresses it"
    );
}

/// Create a branch that starts from `start_point` and deep copies its thresholds.
async fn post_branch_from(
    server: &TestServer,
    fixture: &Fixture,
    name: &str,
    start_point: &str,
) -> serde_json::Value {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({
            "name": name,
            "start_point": { "branch": start_point, "clone_thresholds": true },
        }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    assert_eq!(status, StatusCode::CREATED, "POST branch: {body}");
    serde_json::from_str(&body).expect("Failed to parse the branch")
}

// A start point deep copies every threshold of its branch, and what a threshold
// gates travels with it. One measure may carry several thresholds now, so the clone
// matches on the whole of what each gates and not on the measure alone.
#[tokio::test]
async fn start_point_clone_carries_what_each_threshold_gates() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "clone").await;

    // A bare threshold and a filtered one, both on `latency`, both with a model.
    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size": 512 }),
                &serde_json::json!({ "latency": { "value": FILTERED[0] } }),
            )],
        )],
        Some(threshold_models()),
        None,
    )
    .await;
    create_threshold(
        &server,
        &fixture,
        Some("p99"),
        Some(serde_json::json!([{ "size": 512 }])),
    )
    .await;

    post_branch_from(&server, &fixture, "feature", "main").await;

    let cloned = list_thresholds(&server, &fixture, Some("feature")).await;
    assert_eq!(cloned.len(), 2, "both thresholds cloned: {cloned:?}");

    let bare = threshold_with(&cloned, &serde_json::Value::Null);
    assert_eq!(
        bare["metric"],
        serde_json::Value::Null,
        "the bare threshold arrives bare"
    );
    assert!(bare["model"]["test"].is_string(), "with its model");
    assert_eq!(bare["branch"]["slug"], serde_json::json!("feature"));

    let filtered = threshold_with(&cloned, &serde_json::json!([{ "size": 512 }]));
    assert_eq!(
        filtered["metric"],
        serde_json::json!("p99"),
        "the filtered threshold arrives with the name it gates"
    );
    assert!(filtered["model"]["test"].is_string(), "with its model");
    assert_eq!(filtered["branch"]["slug"], serde_json::json!("feature"));

    // And the start point branch still has exactly what it had.
    let source = list_thresholds(&server, &fixture, Some("main")).await;
    assert_eq!(source.len(), 2, "the start point is unchanged: {source:?}");
}

/// Delete a threshold through the thresholds endpoint.
async fn delete_threshold(
    server: &TestServer,
    fixture: &Fixture,
    threshold: &str,
) -> (StatusCode, String) {
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{threshold}",
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
    (status, body)
}

/// The UUID a threshold response carries.
fn threshold_uuid(threshold: &serde_json::Value) -> String {
    threshold
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the threshold carries its uuid")
        .to_owned()
}

/// One report of one grid point, taken back out again.
///
/// A parameter set is only ever minted by a report, and a report that still
/// references it refuses the delete on its own. Deleting the report leaves the set
/// behind with nothing pointing at it, which is the state where a threshold's claim
/// on it is the only thing left to see.
async fn unreferenced_grid_point(
    server: &TestServer,
    fixture: &Fixture,
    set: &serde_json::Value,
) -> (String, JsonParameter) {
    let json_report = report(
        server,
        fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                set,
                &serde_json::json!({ "latency": { "value": 1.0 } }),
            )],
        )],
        None,
        None,
    )
    .await;
    let report_uuid = json_report
        .get("uuid")
        .and_then(serde_json::Value::as_str)
        .expect("the report carries its uuid")
        .to_owned();

    let benchmark = only_benchmark(server, fixture).await;
    let wanted = parameters(&serde_json::to_string(set).expect("the set serializes"));
    let grid_point = parameter_list(server, fixture, &benchmark, "")
        .await
        .into_iter()
        .find(|parameter| parameter.set == wanted)
        .expect("the reported grid point");

    let (status, body) = delete_report(server, fixture, &report_uuid).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "DELETE report: {body}");

    (benchmark, grid_point)
}

// A threshold that names a parameter set in its filter is a reference to it, so the
// set cannot be deleted out from under it. Deleting the threshold is what makes the
// set deletable, exactly as deleting a report is one level up.
#[tokio::test]
async fn parameter_delete_refuses_while_a_threshold_names_it() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-threshold").await;
    let (benchmark, grid_point) =
        unreferenced_grid_point(&server, &fixture, &serde_json::json!({ "size_mb": 16 })).await;

    let threshold = create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size_mb": 16 }])),
    )
    .await;

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a threshold names the set: {body}"
    );
    assert!(
        body.contains("All thresholds that use this parameter must be deleted first!"),
        "the refusal says what to delete first: {body}"
    );

    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_some(),
        "the refused delete put the parameter set back"
    );
    drop(conn);

    let (status, body) = delete_threshold(&server, &fixture, &threshold_uuid(&threshold)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "DELETE threshold: {body}");

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "nothing names the set any more: {body}"
    );

    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_none(),
        "the parameter set is gone"
    );
}

// Matching a set is not naming it. A filter of `{"size_mb": 16}` matches the grid
// point `{"size_mb": 16, "os": "linux"}` because a filter names only the keys it
// cares about, but it is a predicate over values rather than a reference to that
// row: the grid point can go and the filter still says what it said.
#[tokio::test]
async fn parameter_delete_allows_a_filter_that_only_matches_it() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-subset").await;
    let (benchmark, grid_point) = unreferenced_grid_point(
        &server,
        &fixture,
        &serde_json::json!({ "os": "linux", "size_mb": 16 }),
    )
    .await;

    create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size_mb": 16 }])),
    )
    .await;

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "a filter that merely matches does not stand in the way: {body}"
    );

    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_none(),
        "the parameter set is gone"
    );
}

// When a report and a threshold both point at a set, the report is the one the
// refusal names. The results have to go first either way, and telling a client
// about the threshold while its reports still reference the set would send it to
// the wrong place.
#[tokio::test]
async fn parameter_delete_reports_the_report_reference_first() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "delete-precedence").await;

    report(
        &server,
        &fixture,
        1,
        vec![v1(
            "bench",
            &[entry(
                &serde_json::json!({ "size_mb": 16 }),
                &serde_json::json!({ "latency": { "value": 1.0 } }),
            )],
        )],
        None,
        None,
    )
    .await;

    let benchmark = only_benchmark(&server, &fixture).await;
    let grid_point = parameter_list(&server, &fixture, &benchmark, "")
        .await
        .into_iter()
        .find(|parameter| parameter.set == parameters(r#"{"size_mb":16}"#))
        .expect("the reported grid point");

    create_threshold(
        &server,
        &fixture,
        None,
        Some(serde_json::json!([{ "size_mb": 16 }])),
    )
    .await;

    let (status, body) = delete_parameter(
        &server,
        &fixture,
        &benchmark,
        &fixture.token,
        &grid_point.uuid,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "both point at the set: {body}"
    );
    assert!(
        !body.contains("All thresholds that use this parameter must be deleted first!"),
        "the report reference is the one that fires: {body}"
    );

    let mut conn = server.db_conn();
    assert!(
        parameter_row_id(&mut conn, &grid_point.uuid).is_some(),
        "the parameter set is still there"
    );
}
