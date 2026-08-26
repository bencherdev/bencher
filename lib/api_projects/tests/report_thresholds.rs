#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The thresholds a report payload declares, end to end through ingest.

use bencher_api_tests::{TestServer, helpers::get_project_id};
use bencher_json::{MetricName, ParameterFilter, ParameterSet, ProjectSlug, ThresholdUuid};
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

/// A signed up user with an organization and a project to report into.
async fn fixture(server: &TestServer, label: &str) -> Fixture {
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

    /// Report onto this branch and testbed instead of `main` and `localhost`.
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

/// One BMF v1 payload for a single benchmark's variants.
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

/// What a threshold checks, spelled the way the wire spells it.
///
/// The variants of a threshold that checks every one of them is `*`, and the metric
/// name of a threshold that checks the conventional name is `value`, so the two
/// canonical absences read as what they mean rather than as a hole.
fn dimensions(parameters: Option<ParameterFilter>, metric: Option<MetricName>) -> (String, String) {
    (
        parameters.map_or_else(|| "*".to_owned(), |parameters| parameters.canonical()),
        metric.map_or_else(|| "value".to_owned(), |metric| metric.to_string()),
    )
}

/// Every threshold in a project: what it checks and whether it still carries a model.
fn thresholds(conn: &mut DbConnection, project_id: i32) -> Vec<(String, String, bool)> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select((
            schema::threshold::parameters,
            schema::threshold::metric,
            schema::threshold::model_id,
        ))
        .load::<(Option<ParameterFilter>, Option<MetricName>, Option<i32>)>(conn)
        .expect("Failed to load the thresholds")
        .into_iter()
        .map(|(parameters, metric, model_id)| {
            let (parameters, metric) = dimensions(parameters, metric);
            (parameters, metric, model_id.is_some())
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

/// Every alert in a project, as the variant and metric name it fired on and the
/// dimensions of the threshold that fired it, sorted so the assertion does not depend
/// on detection order.
fn alerts(conn: &mut DbConnection, project_id: i32) -> Vec<(String, String, String, String)> {
    let mut alerts = schema::alert::table
        .inner_join(
            schema::boundary::table
                .inner_join(schema::threshold::table)
                .inner_join(schema::metric::table.inner_join(
                    schema::report_benchmark::table.inner_join(schema::variant::table),
                )),
        )
        .filter(schema::threshold::project_id.eq(project_id))
        .select((
            schema::variant::parameters,
            schema::metric::name,
            schema::threshold::parameters,
            schema::threshold::metric,
        ))
        .load::<(
            ParameterSet,
            MetricName,
            Option<ParameterFilter>,
            Option<MetricName>,
        )>(conn)
        .expect("Failed to load the alerts")
        .into_iter()
        .map(|(set, name, parameters, metric)| {
            let (parameters, metric) = dimensions(parameters, metric);
            (set.canonical(), name.to_string(), parameters, metric)
        })
        .collect::<Vec<_>>();
    alerts.sort();
    alerts
}

/// The point estimates each variant reports, run after run.
const SMALL: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
const LARGE: [f64; 5] = [100.0, 101.0, 102.0, 103.0, 104.0];

/// One steady day of the two variants, each carrying a point estimate and a
/// `p99` beside it.
fn steady(day: usize) -> String {
    let (small, large) = SMALL
        .into_iter()
        .zip(LARGE)
        .nth(day % SMALL.len())
        .expect("the day is one of the steady ones");
    variants(small, large)
}

fn variants(small: f64, large: f64) -> String {
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

/// The threshold entries a version 1 payload declares: one that checks `p99` on
/// every variant, and one that checks the point estimate of a single variant.
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
// created them is checked by them.
#[tokio::test]
async fn v1_entries_create_thresholds_that_check_the_same_report() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "entries").await;

    for day in 0..SMALL.len() {
        report(&server, &fixture, Post::new(day + 1, vec![steady(day)])).await;
    }

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert!(
        thresholds(&mut conn, project_id).is_empty(),
        "nothing checked the history"
    );
    drop(conn);

    // Both variants jump tenfold on both names.
    report(
        &server,
        &fixture,
        Post::new(6, vec![variants(1_000.0, 10_000.0)]).thresholds(entries()),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("*".to_owned(), "p99".to_owned(), true),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), true),
        ],
        "each entry created the threshold it named"
    );

    assert_eq!(
        alerts(&mut conn, project_id),
        vec![
            (
                r#"{"size_mb":16}"#.to_owned(),
                "p99".to_owned(),
                "*".to_owned(),
                "p99".to_owned(),
            ),
            (
                r#"{"size_mb":16}"#.to_owned(),
                "value".to_owned(),
                r#"[{"size_mb":16}]"#.to_owned(),
                "value".to_owned(),
            ),
            (
                r#"{"size_mb":32}"#.to_owned(),
                "p99".to_owned(),
                "*".to_owned(),
                "p99".to_owned(),
            ),
        ],
        "the named threshold fires on both variants, and the filtered one fires on its own: the large variant's point estimate jumped just as far and nobody checks it"
    );
}

// The filtered threshold checks its variant and no other. The large variant's
// point estimate is an outlier against its own history and raises nothing, because
// no threshold checks it.
#[tokio::test]
async fn a_filtered_threshold_checks_only_its_variants() {
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
        Post::new(6, vec![variants(1_000.0, 10_000.0)]).thresholds(thresholds_json),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        alerts(&mut conn, project_id),
        vec![(
            r#"{"size_mb":16}"#.to_owned(),
            "value".to_owned(),
            r#"[{"size_mb":16}]"#.to_owned(),
            "value".to_owned(),
        )],
        "the other variant's tenfold jump is nobody's business"
    );
}

// The same dimensions declared by two reports update the one threshold rather than
// creating a second.
#[tokio::test]
async fn one_set_of_dimensions_across_two_reports_is_one_threshold() {
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
    // The same dimensions spelled differently: the filter's sets are canonical, so
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

// A version 1 list that names one set of dimensions twice, in two spellings, makes one
// threshold at the first entry's position with the last entry's model.
#[tokio::test]
async fn duplicate_dimensions_in_one_payload_keep_the_last() {
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
                { "measure": "latency", "metric": "p99", "model": model() },
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
        vec![
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), true),
            ("*".to_owned(), "p99".to_owned(), true),
        ],
        "one set of dimensions is one threshold, and the filtered threshold sits where it was first written"
    );
    let filtered = threshold_ids(&mut conn, project_id)
        .first()
        .copied()
        .expect("the filtered threshold");
    let boundaries = schema::model::table
        .filter(schema::model::threshold_id.eq(filtered))
        .select(schema::model::upper_boundary)
        .load::<Option<f64>>(&mut conn)
        .expect("Failed to load the models");
    assert_eq!(
        boundaries,
        vec![Some(0.25)],
        "the last entry is the model that was written"
    );
}

#[tokio::test]
async fn every_spelling_of_the_bare_threshold_is_one_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "spellings").await;
    let project_id = get_project_id(&server, fixture.project_slug.as_ref());

    let entries = [
        serde_json::json!({
            "measure": "latency",
            "model": { "test": "percentage", "upper_boundary": 0.1 },
        }),
        serde_json::json!({
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.2 },
        }),
        serde_json::json!({
            "parameters": [],
            "measure": "latency",
            "model": { "test": "percentage", "upper_boundary": 0.3 },
        }),
        serde_json::json!({
            "parameters": [{}],
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.4 },
        }),
    ];
    for (day, entry) in entries.into_iter().enumerate() {
        let spelled = entry.to_string();
        let sent = entry["model"]["upper_boundary"].as_f64();
        report(
            &server,
            &fixture,
            Post::new(day + 1, vec![steady(day)])
                .thresholds(serde_json::json!({ "models": [entry] })),
        )
        .await;

        let mut conn = server.db_conn();
        assert_eq!(
            thresholds(&mut conn, project_id),
            vec![("*".to_owned(), "value".to_owned(), true)],
            "{spelled} addresses the bare threshold"
        );
        let model_id = schema::threshold::table
            .filter(schema::threshold::project_id.eq(project_id))
            .select(schema::threshold::model_id)
            .first::<Option<i32>>(&mut conn)
            .expect("Failed to load the threshold")
            .expect("the bare threshold has a model");
        let upper_boundary = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .select(schema::model::upper_boundary)
            .first::<Option<f64>>(&mut conn)
            .expect("Failed to load the current model");
        assert_eq!(
            upper_boundary, sent,
            "{spelled} is the bare threshold's current model"
        );
    }
}

// A version 0 payload still carries the map, and the map still addresses the bare
// threshold: the conventional name of every variant.
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
        vec![("*".to_owned(), "value".to_owned(), true)],
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
    let refusal = serde_json::from_str::<serde_json::Value>(&body).expect("the refusal is JSON");
    assert_eq!(
        refusal["message"],
        "The report is read as BMF version 0, so `thresholds.models` must be a map of measure to threshold model, but a list was sent.",
        "the refusal names the version: {body}"
    );

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
        "the shape is refused before the report's dimensions are created"
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
    let refusal = serde_json::from_str::<serde_json::Value>(&body).expect("the refusal is JSON");
    assert_eq!(
        refusal["message"],
        "The report is read as BMF version 1, so `thresholds.models` must be a list of threshold entries, but a map was sent.",
        "the refusal names the version: {body}"
    );

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        (
            branch_count(&mut conn, project_id, "refused-branch"),
            testbed_count(&mut conn, project_id, "refused-testbed"),
        ),
        (0, 0),
        "the shape is refused before the report's dimensions are created"
    );
}

// An empty map is still a map, so it is refused at version 1 for the same reason a
// full one is.
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
    let refusal = serde_json::from_str::<serde_json::Value>(&body).expect("the refusal is JSON");
    assert_eq!(
        refusal["message"],
        "The report is read as BMF version 1, so `thresholds.models` must be a list of threshold entries, but a map was sent.",
        "the refusal names the version: {body}"
    );
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
            ("*".to_owned(), "value".to_owned(), true),
            ("*".to_owned(), "p99".to_owned(), true),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), true),
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
            ("*".to_owned(), "value".to_owned(), false),
            ("*".to_owned(), "p99".to_owned(), true),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), true),
        ],
        "only the bare threshold is addressable at version 0"
    );
}

// A version 1 list addresses every set of dimensions, so `reset` reaches every threshold it
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
            ("*".to_owned(), "value".to_owned(), false),
            ("*".to_owned(), "p99".to_owned(), true),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), false),
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
            ("*".to_owned(), "value".to_owned(), false),
            ("*".to_owned(), "p99".to_owned(), false),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), false),
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

    let bare = serde_json::json!({ "models": [{ "measure": "latency", "model": model() }] });
    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)])
            .onto("other", "localhost")
            .thresholds(bare.clone()),
    )
    .await;
    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)])
            .onto("main", "other")
            .thresholds(bare),
    )
    .await;

    report(
        &server,
        &fixture,
        Post::new(3, vec![steady(2)]).thresholds(serde_json::json!({ "reset": true })),
    )
    .await;

    let project_id = get_project_id(&server, fixture.project_slug.as_ref());
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![
            ("*".to_owned(), "value".to_owned(), true),
            ("*".to_owned(), "value".to_owned(), true),
        ],
        "the other branch's threshold and the other testbed's threshold keep their models"
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
            ("*".to_owned(), "value".to_owned(), true),
            ("*".to_owned(), "value".to_owned(), false),
            ("*".to_owned(), "p99".to_owned(), true),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), true),
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

// A `models` that is neither shape is refused by naming both.
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
