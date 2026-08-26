#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The thresholds a report payload declares, end to end through ingest.
//!
//! A report has always carried a map of measure to model, and a map key names a
//! measure and nothing else. BMF version 1 carries a list instead, and a list entry
//! names everything a threshold gates: the grid points, the measure, and the metric
//! name. The version the payload declares is what says which shape it is written
//! in, so the two shapes never have to be told apart by looking at them.
//!
//! Two things follow from that and are what most of this file is about. A threshold
//! the report creates gates the very report that created it, because thresholds are
//! resolved before results are. And `reset` reaches exactly as far as the shape can
//! address: a version 0 map can only name bare thresholds, so a legacy run cannot
//! strip a filtered threshold it cannot even spell.

use bencher_api_tests::{TestServer, helpers::get_project_id};
use bencher_json::{
    BmfVersion, MetricName, ParameterFilter, ParameterSet, ProjectSlug, ThresholdUuid,
};
use bencher_schema::{context::DbConnection, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// A threshold model loose enough to compute a boundary from a short history and
/// tight enough that a tenfold jump is an outlier.
fn model() -> serde_json::Value {
    serde_json::json!({
        "test": "t_test",
        "min_sample_size": 2,
        "max_sample_size": 64,
        "lower_boundary": 0.98,
        "upper_boundary": 0.98,
    })
}

/// A signed up user with an organization and a project to report into.
struct Fixture {
    project_slug: ProjectSlug,
    token: String,
}

/// A project whose BMF version gate is still at 0, which is where every project
/// starts.
async fn ungated_fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("rt{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("RT Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("RT Project {label}"))
        .await;
    Fixture {
        project_slug: project.slug,
        token: user.token,
    }
}

/// A project that accepts BMF version 1, which every payload here declares.
async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let fixture = ungated_fixture(server, label).await;
    server.set_bmf_version(&fixture.project_slug, BmfVersion::V1);
    fixture
}

/// One report request.
///
/// `day` only has to be distinct and increasing, so reports order the way they were
/// submitted.
struct Post {
    day: usize,
    results: Vec<String>,
    bmf_version: u8,
    thresholds: Option<serde_json::Value>,
    branch: &'static str,
    testbed: &'static str,
}

impl Post {
    fn new(day: usize, results: Vec<String>) -> Self {
        Self {
            day,
            results,
            bmf_version: 1,
            thresholds: None,
            branch: "main",
            testbed: "localhost",
        }
    }

    fn v0(mut self) -> Self {
        self.bmf_version = 0;
        self
    }

    fn thresholds(mut self, thresholds: serde_json::Value) -> Self {
        self.thresholds = Some(thresholds);
        self
    }

    /// Report onto a branch and testbed the project does not have yet, so that
    /// whether they exist afterwards says whether the request got as far as
    /// creating anything.
    fn onto(mut self, branch: &'static str, testbed: &'static str) -> Self {
        self.branch = branch;
        self.testbed = testbed;
        self
    }
}

/// Post one report and return its status and body, whatever they are.
async fn try_report(server: &TestServer, fixture: &Fixture, post: Post) -> (StatusCode, String) {
    let Post {
        day,
        results,
        bmf_version,
        thresholds,
        branch,
        testbed,
    } = post;
    let body = serde_json::json!({
        "branch": branch,
        "testbed": testbed,
        "start_time": format!("2024-01-{day:02}T00:00:00Z"),
        "end_time": format!("2024-01-{day:02}T00:01:00Z"),
        "results": results,
        "bmf_version": bmf_version,
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
    (status, body)
}

/// Post one report and require it to be created.
async fn report(server: &TestServer, fixture: &Fixture, post: Post) {
    let day = post.day;
    let (status, body) = try_report(server, fixture, post).await;
    assert_eq!(status, StatusCode::CREATED, "POST report {day}: {body}");
}

/// One BMF v1 payload for a single benchmark's grid points.
fn v1(entries: &[serde_json::Value]) -> String {
    serde_json::to_string(&serde_json::json!({ "bench": entries })).expect("the results serialize")
}

fn entry(parameters: &serde_json::Value, measures: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "parameters": parameters, "measures": measures })
}

/// One BMF v0 payload for a single benchmark.
fn v0(value: f64) -> String {
    serde_json::json!({ "bench": { "latency": { "value": value } } }).to_string()
}

/// What a threshold gates, spelled the way the wire spells it.
///
/// The metric name of a threshold that gates the conventional name is `value`, and
/// the grid points of a threshold that gates every one of them is `*`, so the two
/// canonical absences read as what they mean rather than as a hole.
fn identity(metric: Option<MetricName>, parameters: Option<ParameterFilter>) -> (String, String) {
    (
        metric.map_or_else(|| "value".to_owned(), |metric| metric.to_string()),
        parameters.map_or_else(|| "*".to_owned(), |parameters| parameters.canonical()),
    )
}

/// Every threshold in a project: what it gates and whether it still carries a model.
fn thresholds(conn: &mut DbConnection, project_id: i32) -> Vec<(String, String, bool)> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select((
            schema::threshold::metric,
            schema::threshold::parameters,
            schema::threshold::model_id,
        ))
        .load::<(Option<MetricName>, Option<ParameterFilter>, Option<i32>)>(conn)
        .expect("Failed to load the thresholds")
        .into_iter()
        .map(|(metric, parameters, model_id)| {
            let (metric, parameters) = identity(metric, parameters);
            (metric, parameters, model_id.is_some())
        })
        .collect()
}

/// How many branches of a project carry this name.
fn branch_count(conn: &mut DbConnection, project_id: i32, name: &str) -> i64 {
    schema::branch::table
        .filter(schema::branch::project_id.eq(project_id))
        .filter(schema::branch::name.eq(name))
        .count()
        .get_result(conn)
        .expect("Failed to count the branches")
}

/// How many testbeds of a project carry this name.
fn testbed_count(conn: &mut DbConnection, project_id: i32, name: &str) -> i64 {
    schema::testbed::table
        .filter(schema::testbed::project_id.eq(project_id))
        .filter(schema::testbed::name.eq(name))
        .count()
        .get_result(conn)
        .expect("Failed to count the testbeds")
}

/// Every threshold row id in a project, in creation order.
fn threshold_ids(conn: &mut DbConnection, project_id: i32) -> Vec<i32> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select(schema::threshold::id)
        .load::<i32>(conn)
        .expect("Failed to load the threshold ids")
}

/// Every threshold UUID in a project, in creation order.
fn threshold_uuids(conn: &mut DbConnection, project_id: i32) -> Vec<ThresholdUuid> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select(schema::threshold::uuid)
        .load::<ThresholdUuid>(conn)
        .expect("Failed to load the threshold uuids")
}

/// Every alert in a project, as the grid point and metric name it fired on and the
/// identity of the threshold that fired it, sorted so the assertion does not depend
/// on detection order.
fn alerts(conn: &mut DbConnection, project_id: i32) -> Vec<(String, String, String, String)> {
    let mut alerts = schema::alert::table
        .inner_join(
            schema::boundary::table
                .inner_join(schema::threshold::table)
                .inner_join(schema::metric::table.inner_join(
                    schema::report_benchmark::table.inner_join(schema::parameter::table),
                )),
        )
        .filter(schema::threshold::project_id.eq(project_id))
        .select((
            schema::parameter::set,
            schema::metric::name,
            schema::threshold::metric,
            schema::threshold::parameters,
        ))
        .load::<(
            ParameterSet,
            MetricName,
            Option<MetricName>,
            Option<ParameterFilter>,
        )>(conn)
        .expect("Failed to load the alerts")
        .into_iter()
        .map(|(set, name, metric, parameters)| {
            let (metric, parameters) = identity(metric, parameters);
            (set.canonical(), name.to_string(), metric, parameters)
        })
        .collect::<Vec<_>>();
    alerts.sort();
    alerts
}

/// The point estimates each grid point reports, run after run.
const SMALL: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const LARGE: [f64; 5] = [100.0, 101.0, 102.0, 103.0, 104.0];

/// One steady day of the two grid points, each carrying a point estimate and a
/// `p99` beside it.
fn steady(day: usize) -> String {
    let (small, large) = SMALL
        .into_iter()
        .zip(LARGE)
        .nth(day % SMALL.len())
        .expect("the day is one of the steady ones");
    grid(small, large)
}

fn grid(small: f64, large: f64) -> String {
    v1(&[
        entry(
            &serde_json::json!({ "size_mb": 16 }),
            &serde_json::json!({ "latency": { "value": small, "p99": small + 1.0 } }),
        ),
        entry(
            &serde_json::json!({ "size_mb": 32 }),
            &serde_json::json!({ "latency": { "value": large, "p99": large + 1.0 } }),
        ),
    ])
}

/// The threshold entries a version 1 payload declares: one that gates `p99` on
/// every grid point, and one that gates the point estimate of a single grid point.
fn entries() -> serde_json::Value {
    serde_json::json!({
        "models": [
            { "measure": "latency", "metric": "p99", "model": model() },
            {
                "parameters": [{ "size_mb": 16 }],
                "measure": "latency",
                "model": model(),
            },
        ]
    })
}

// A version 1 entry list creates the thresholds it names and the very report that
// created them is gated by them.
//
// The history is five reports that declared no threshold at all, so nothing was
// gated until the sixth report both declared the entries and carried the outliers.
// Every alert here belongs to a threshold that did not exist when the request
// arrived, which is what "thresholds are resolved before results" buys.
#[tokio::test]
async fn v1_entries_create_thresholds_that_gate_the_same_report() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "entries").await;

    for day in 0..SMALL.len() {
        report(&server, &fixture, Post::new(day + 1, vec![steady(day)])).await;
    }

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert!(
        thresholds(&mut conn, project_id).is_empty(),
        "nothing gated the history"
    );
    drop(conn);

    // Both grid points jump tenfold on both names.
    report(
        &server,
        &fixture,
        Post::new(6, vec![grid(1_000.0, 10_000.0)]).thresholds(entries()),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("p99".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), true),
        ],
        "each entry created the threshold it named"
    );

    assert_eq!(
        alerts(&mut conn, project_id),
        vec![
            (
                r#"{"size_mb":16}"#.to_owned(),
                "p99".to_owned(),
                "p99".to_owned(),
                "*".to_owned(),
            ),
            (
                r#"{"size_mb":16}"#.to_owned(),
                "value".to_owned(),
                "value".to_owned(),
                r#"[{"size_mb":16}]"#.to_owned(),
            ),
            (
                r#"{"size_mb":32}"#.to_owned(),
                "p99".to_owned(),
                "p99".to_owned(),
                "*".to_owned(),
            ),
        ],
        "the named threshold fires on both grid points, and the filtered one fires on its own: the large grid point's point estimate jumped just as far and nobody gates it"
    );
}

// The filtered threshold gates its grid point and no other. The large grid point's
// point estimate is an outlier against its own history and raises nothing, because
// no threshold gates it.
#[tokio::test]
async fn a_filtered_threshold_gates_only_its_grid_points() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "filtered").await;

    let thresholds_json = serde_json::json!({
        "models": [{
            "parameters": [{ "size_mb": 16 }],
            "measure": "latency",
            "model": model(),
        }]
    });
    for day in 0..SMALL.len() {
        report(
            &server,
            &fixture,
            Post::new(day + 1, vec![steady(day)]).thresholds(thresholds_json.clone()),
        )
        .await;
    }
    report(
        &server,
        &fixture,
        Post::new(6, vec![grid(1_000.0, 10_000.0)]).thresholds(thresholds_json),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        alerts(&mut conn, project_id),
        vec![(
            r#"{"size_mb":16}"#.to_owned(),
            "value".to_owned(),
            "value".to_owned(),
            r#"[{"size_mb":16}]"#.to_owned(),
        )],
        "the other grid point's tenfold jump is nobody's business"
    );
}

// The same identity declared by two reports updates the one threshold rather than
// creating a second.
#[tokio::test]
async fn one_identity_across_two_reports_is_one_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "update").await;

    let first = serde_json::json!({
        "models": [{
            "parameters": [{ "size_mb": 16 }],
            "measure": "latency",
            "metric": "p99",
            "model": model(),
        }]
    });
    // The same identity spelled differently: the filter's sets are canonical, so
    // this is the same threshold with a different model.
    let second = serde_json::json!({
        "models": [{
            "parameters": [{ "size_mb": 16.0 }],
            "measure": "latency",
            "metric": "p99",
            "model": {
                "test": "percentage",
                "upper_boundary": 0.25,
            },
        }]
    });

    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(first),
    )
    .await;
    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    let created = threshold_uuids(&mut conn, project_id);
    let created_ids = threshold_ids(&mut conn, project_id);
    assert_eq!(created.len(), 1, "the entry created one threshold");
    drop(conn);

    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(second),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        threshold_uuids(&mut conn, project_id),
        created,
        "the second report updated the threshold the first created"
    );
    let models = schema::model::table
        .filter(schema::model::threshold_id.eq_any(&created_ids))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("Failed to count the models");
    assert_eq!(models, 2, "the new model sits beside the replaced one");
}

// One payload may name one identity twice, and the last entry is the one that
// counts. Two spellings of one filter are one identity, so this is a duplicate even
// though the two entries do not read alike.
#[tokio::test]
async fn a_duplicate_identity_in_one_payload_keeps_the_last() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "duplicate").await;

    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                {
                    "parameters": [{ "size_mb": 16 }, { "size_mb": 16 }],
                    "measure": "latency",
                    "model": model(),
                },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "model": { "test": "percentage", "upper_boundary": 0.25 },
                },
            ]
        })),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), true)],
        "one identity is one threshold"
    );
    let boundaries = schema::model::table
        .filter(schema::model::threshold_id.eq_any(threshold_ids(&mut conn, project_id)))
        .select(schema::model::upper_boundary)
        .load::<Option<f64>>(&mut conn)
        .expect("Failed to load the models");
    assert_eq!(
        boundaries,
        vec![Some(0.25)],
        "the last entry is the model that was written"
    );
}

// A version 0 payload still carries the map, and the map still addresses the bare
// threshold: the conventional name of every grid point.
#[tokio::test]
async fn v0_map_creates_a_bare_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0map").await;

    report(
        &server,
        &fixture,
        Post::new(1, vec![v0(10.0)])
            .v0()
            .thresholds(serde_json::json!({ "models": { "latency": model() } })),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![("value".to_owned(), "*".to_owned(), true)],
        "a map key addresses the bare threshold"
    );
}

// The shape is not guessed at. A payload that declares a version and then sends the
// other version's shape is refused, and the refusal names the version it declared.
#[tokio::test]
async fn a_list_at_version_0_is_a_bad_request() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "listv0").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![v0(10.0)])
            .v0()
            .onto("refused-branch", "refused-testbed")
            .thresholds(entries()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a list at version 0: {body}"
    );
    assert!(body.contains('0'), "the refusal names the version: {body}");

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert!(
        thresholds(&mut conn, project_id).is_empty(),
        "the refused payload created no threshold"
    );
    assert_eq!(
        (
            branch_count(&mut conn, project_id, "refused-branch"),
            testbed_count(&mut conn, project_id, "refused-testbed"),
        ),
        (0, 0),
        "the shape is refused beside the version gate, before the report's dimensions are created"
    );
}

#[tokio::test]
async fn a_map_at_version_1_is_a_bad_request() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "mapv1").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)])
            .onto("refused-branch", "refused-testbed")
            .thresholds(serde_json::json!({ "models": { "latency": model() } })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a map at version 1: {body}"
    );
    assert!(body.contains('1'), "the refusal names the version: {body}");

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        (
            branch_count(&mut conn, project_id, "refused-branch"),
            testbed_count(&mut conn, project_id, "refused-testbed"),
        ),
        (0, 0),
        "the shape is refused beside the version gate, before the report's dimensions are created"
    );
}

// An empty map is still a map, so it is refused at version 1 for the same reason a
// full one is. The shape is checked before anything the payload says is acted on.
#[tokio::test]
async fn an_empty_map_at_version_1_is_a_bad_request() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "emptymap").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)])
            .thresholds(serde_json::json!({ "models": {}, "reset": true })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "an empty map: {body}");
}

// The project gate is checked before the shape is, because the gate is checked
// before anything at all is created for the report. A project that does not accept
// version 1 turns the payload away for that reason and never gets as far as reading
// the list.
#[tokio::test]
async fn a_list_on_an_ungated_project_is_the_gate_refusal() {
    let server = TestServer::new().await;
    let fixture = ungated_fixture(&server, "ungated").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![v0(10.0)]).thresholds(entries()),
    )
    .await;
    assert_eq!(status, StatusCode::LOCKED, "the gate refuses first: {body}");
}

/// A project holding three thresholds: a bare one, a named one, and a filtered one,
/// each with a model.
async fn three_thresholds(server: &TestServer, label: &str) -> (Fixture, i32) {
    let fixture = fixture(server, label).await;
    report(
        server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "model": model() },
                { "measure": "latency", "metric": "p99", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "model": model(),
                },
            ]
        })),
    )
    .await;
    let project_id = get_project_id(server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("value".to_owned(), "*".to_owned(), true),
            ("p99".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), true),
        ],
        "the fixture holds one threshold of each kind"
    );
    drop(conn);
    (fixture, project_id)
}

// `reset` reaches as far as the payload's shape can address and no further. A
// version 0 map can only name the bare threshold, so that is the only model it
// takes away: a legacy pipeline cannot strip a threshold it has no way to spell.
#[tokio::test]
async fn v0_reset_leaves_the_named_and_filtered_thresholds_standing() {
    let server = TestServer::new().await;
    let (fixture, project_id) = three_thresholds(&server, "v0reset").await;

    report(
        &server,
        &fixture,
        Post::new(2, vec![v0(11.0)])
            .v0()
            .thresholds(serde_json::json!({ "reset": true })),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("value".to_owned(), "*".to_owned(), false),
            ("p99".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), true),
        ],
        "only the bare threshold is addressable at version 0"
    );
}

// A version 1 list addresses every identity, so `reset` reaches every threshold it
// did not name.
#[tokio::test]
async fn v1_reset_strips_what_the_entries_did_not_name() {
    let server = TestServer::new().await;
    let (fixture, project_id) = three_thresholds(&server, "v1reset").await;

    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({
            "models": [{ "measure": "latency", "metric": "p99", "model": model() }],
            "reset": true,
        })),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("value".to_owned(), "*".to_owned(), false),
            ("p99".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), false),
        ],
        "the named threshold keeps its model and the rest lose theirs"
    );
}

// A version 1 payload that names nothing at all still addresses everything, so a
// bare `reset` strips every threshold on the branch and testbed.
#[tokio::test]
async fn v1_reset_without_entries_strips_every_threshold() {
    let server = TestServer::new().await;
    let (fixture, project_id) = three_thresholds(&server, "v1resetall").await;

    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({ "reset": true })),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("value".to_owned(), "*".to_owned(), false),
            ("p99".to_owned(), "*".to_owned(), false),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), false),
        ],
        "a list that names nothing still addresses everything"
    );
}

// A threshold on another branch or testbed is not on this report's, so `reset` at
// version 1 does not reach it either.
#[tokio::test]
async fn v1_reset_stays_on_the_report_branch_and_testbed() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "resetscope").await;

    let body = serde_json::json!({
        "branch": "other",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [steady(0)],
        "bmf_version": 1,
        "thresholds": { "models": [{ "measure": "latency", "model": model() }] },
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
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "POST the other branch's report"
    );

    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({ "reset": true })),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![("value".to_owned(), "*".to_owned(), true)],
        "the other branch's threshold keeps its model"
    );
}

// A version 0 map and `reset` in one payload, which is what a pipeline running
// `bencher run --thresholds-reset` sends today. The map updates the bare threshold
// of the measure it names, `reset` takes the model away from the bare threshold it
// did not name, and the named and filtered thresholds it cannot address are left
// exactly as they were.
#[tokio::test]
async fn v0_map_with_reset_updates_what_it_names_and_strips_the_rest() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0mapreset").await;

    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "model": model() },
                { "measure": "throughput", "model": model() },
                { "measure": "latency", "metric": "p99", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "model": model(),
                },
            ]
        })),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    let latency = threshold_ids(&mut conn, project_id)
        .first()
        .copied()
        .expect("the bare latency threshold");
    drop(conn);

    report(
        &server,
        &fixture,
        Post::new(2, vec![v0(11.0)])
            .v0()
            .thresholds(serde_json::json!({
                "models": { "latency": { "test": "percentage", "upper_boundary": 0.25 } },
                "reset": true,
            })),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("value".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), "*".to_owned(), false),
            ("p99".to_owned(), "*".to_owned(), true),
            ("value".to_owned(), r#"[{"size_mb":16}]"#.to_owned(), true),
        ],
        "the map keeps the measure it named, reset strips the bare threshold it did not, and the rest are out of reach"
    );

    let models = schema::model::table
        .filter(schema::model::threshold_id.eq(latency))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("Failed to count the models");
    assert_eq!(models, 2, "the map updated the model it named");
}

// What a client is told when the thresholds it sent are malformed rather than the
// wrong shape for its version.
//
// Two shapes behind one key is a place where error quality quietly dies: the obvious
// spelling, an untagged enum, buffers the input, tries each variant, and reports only
// that nothing matched, so a misspelled model test comes back as "data did not match
// any variant" instead of naming the field and the variants it could have been. The
// shape is known from the first token, so it is decided by looking rather than by
// trying, and the map's own errors and the list's own errors are what a client reads.
//
// These pin the two halves of that: the version 0 map's message is exactly the one it
// was before this layer existed, and the version 1 list's message points at the entry
// and the field inside it.
#[tokio::test]
async fn a_malformed_v0_map_names_the_field_that_is_wrong() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "badmap").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![v0(10.0)])
            .v0()
            .thresholds(serde_json::json!({ "models": { "latency": { "test": "bogus" } } })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a bogus test: {body}");
    assert!(
        body.contains("thresholds.models.latency.test"),
        "the refusal names the field, not just the payload: {body}"
    );
    assert!(
        body.contains("unknown variant `bogus`"),
        "the refusal names what was wrong with it: {body}"
    );
    assert!(
        body.contains("`t_test`"),
        "the refusal lists what it could have been: {body}"
    );
}

#[tokio::test]
async fn a_malformed_v1_entry_names_the_entry_and_the_field() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "badentry").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [{ "measure": "latency", "model": { "test": "bogus" } }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a bogus test: {body}");
    assert!(
        body.contains("thresholds.models[0].model.test"),
        "the refusal names the entry and the field inside it: {body}"
    );
    assert!(
        body.contains("unknown variant `bogus`"),
        "the refusal names what was wrong with it: {body}"
    );

    // A missing field is named the same way, by the entry it is missing from.
    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({
            "models": [{ "model": { "test": "static" } }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a missing measure: {body}");
    assert!(
        body.contains("thresholds.models[0]") && body.contains("missing field `measure`"),
        "the refusal names the entry and the field it wants: {body}"
    );
}

// A `models` that is neither shape is refused by naming both, which is the one
// message this layer changes: the field used to accept only a map and now accepts
// either, so what it expects has to say so.
#[tokio::test]
async fn a_models_field_that_is_neither_shape_names_both() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "neither").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({ "models": 7 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "neither shape: {body}");
    assert!(
        body.contains("thresholds.models") && body.contains("invalid type: integer `7`"),
        "the refusal names the field and what arrived: {body}"
    );
    assert!(
        body.contains("a map of measure to threshold model")
            && body.contains("a list of threshold entries"),
        "the refusal names both shapes it would have taken: {body}"
    );
}
