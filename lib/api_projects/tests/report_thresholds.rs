#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The thresholds a report payload declares, end to end through ingest.

use bencher_api_tests::{TestServer, helpers::get_project_id};
use bencher_json::{
    BmfVersion, JsonReport, MeasureUuid, MetricName, ParameterFilter, ParameterSet, ProjectSlug,
    ReportUuid, ThresholdUuid,
    project::{
        report::{JsonReportWarning, ReportWarningAction, ReportWarningResource},
        threshold::MAX_ACTIVE_THRESHOLDS,
    },
};
use bencher_schema::{
    context::DbConnection,
    model::project::{
        ProjectId,
        threshold::{ThresholdId, model::ModelId},
    },
    schema,
};
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

/// The fixture project's row id.
fn project_id(server: &TestServer, fixture: &Fixture) -> ProjectId {
    ProjectId::try_from_raw(get_project_id(server, fixture.project_slug.as_ref()))
        .expect("valid project ID")
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
    bmf_version: Option<u8>,
    thresholds: Option<serde_json::Value>,
    branch: &'static str,
    testbed: &'static str,
}

impl Post {
    fn new(day: usize, results: Vec<String>) -> Self {
        Self {
            day,
            results,
            bmf_version: Some(1),
            thresholds: None,
            branch: "main",
            testbed: "localhost",
        }
    }

    fn v0(mut self) -> Self {
        self.bmf_version = Some(0);
        self
    }

    /// Declare no version, so the report is read at the project's.
    fn undeclared(mut self) -> Self {
        self.bmf_version = None;
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
async fn report(server: &TestServer, fixture: &Fixture, post: Post) -> JsonReport {
    let day = post.day;
    let (status, body) = try_report(server, fixture, post).await;
    assert_eq!(status, StatusCode::CREATED, "POST report {day}: {body}");
    serde_json::from_str(&body).expect("Failed to parse the report")
}

/// Read one report back.
async fn get_report(server: &TestServer, fixture: &Fixture, report: ReportUuid) -> JsonReport {
    let resp = server
        .client
        .get(server.api_url(&format!(
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
    assert_eq!(resp.status(), StatusCode::OK, "GET report {report}");
    resp.json().await.expect("Failed to parse the report")
}

/// The one warning of a report that skipped `count` thresholds.
fn skipped_thresholds(count: u32) -> Vec<JsonReportWarning> {
    vec![JsonReportWarning {
        resource: ReportWarningResource::Threshold,
        action: ReportWarningAction::Skip,
        count,
    }]
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
fn thresholds(conn: &mut DbConnection, project_id: ProjectId) -> Vec<(String, String, bool)> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select((
            schema::threshold::parameters,
            schema::threshold::metric,
            schema::threshold::model_id,
        ))
        .load::<(Option<ParameterFilter>, Option<MetricName>, Option<ModelId>)>(conn)
        .expect("Failed to load the thresholds")
        .into_iter()
        .map(|(parameters, metric, model_id)| {
            let (parameters, metric) = dimensions(parameters, metric);
            (parameters, metric, model_id.is_some())
        })
        .collect()
}

/// How many branches of a project carry this name.
fn branch_count(conn: &mut DbConnection, project_id: ProjectId, name: &str) -> i64 {
    schema::branch::table
        .filter(schema::branch::project_id.eq(project_id))
        .filter(schema::branch::name.eq(name))
        .count()
        .get_result(conn)
        .expect("Failed to count the branches")
}

/// How many testbeds of a project carry this name.
fn testbed_count(conn: &mut DbConnection, project_id: ProjectId, name: &str) -> i64 {
    schema::testbed::table
        .filter(schema::testbed::project_id.eq(project_id))
        .filter(schema::testbed::name.eq(name))
        .count()
        .get_result(conn)
        .expect("Failed to count the testbeds")
}

/// Every threshold row id in a project, in creation order.
fn threshold_ids(conn: &mut DbConnection, project_id: ProjectId) -> Vec<ThresholdId> {
    schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .select(schema::threshold::id)
        .load::<ThresholdId>(conn)
        .expect("Failed to load the threshold ids")
}

/// Every threshold UUID in a project, in creation order.
fn threshold_uuids(conn: &mut DbConnection, project_id: ProjectId) -> Vec<ThresholdUuid> {
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
fn alerts(conn: &mut DbConnection, project_id: ProjectId) -> Vec<(String, String, String, String)> {
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
                "metric": "value",
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

    let project_id = project_id(&server, &fixture);
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
            "metric": "value",
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

    let project_id = project_id(&server, &fixture);
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
    let project_id = project_id(&server, &fixture);
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
                    "metric": "value",
                    "model": model(),
                },
                { "measure": "latency", "metric": "p99", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "metric": "value",
                    "model": { "test": "percentage", "upper_boundary": 0.25 },
                },
            ]
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
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

// A version 0 map key makes the bare threshold, and every version 1 spelling of it
// updates that one.
#[tokio::test]
async fn every_spelling_of_the_bare_threshold_is_one_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "spellings").await;
    let project_id = project_id(&server, &fixture);

    report(
        &server,
        &fixture,
        Post::new(1, vec![v0(10.0)])
            .v0()
            .thresholds(serde_json::json!({
                "models": { "latency": { "test": "percentage", "upper_boundary": 0.1 } }
            })),
    )
    .await;

    let entries = [
        serde_json::json!({
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.2 },
        }),
        serde_json::json!({
            "parameters": [],
            "measure": "latency",
            "metric": "value",
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
            Post::new(day + 2, vec![steady(day)])
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
            .first::<Option<ModelId>>(&mut conn)
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

    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![("*".to_owned(), "value".to_owned(), true)],
        "a map key addresses the bare threshold"
    );
}

// Two map keys that resolve to one measure, its name and its UUID, address one threshold.
#[tokio::test]
async fn v0_map_keys_for_one_measure_are_one_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0mapsame").await;

    report(&server, &fixture, Post::new(1, vec![v0(10.0)]).v0()).await;
    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    let latency = schema::measure::table
        .filter(schema::measure::project_id.eq(project_id))
        .filter(schema::measure::slug.eq("latency"))
        .select(schema::measure::uuid)
        .first::<MeasureUuid>(&mut conn)
        .expect("Failed to load the measure");
    drop(conn);

    let models = [
        ("latency".to_owned(), model()),
        (latency.to_string(), model()),
    ]
    .into_iter()
    .collect::<serde_json::Map<_, _>>();
    report(
        &server,
        &fixture,
        Post::new(2, vec![v0(11.0)])
            .v0()
            .thresholds(serde_json::json!({ "models": models })),
    )
    .await;

    let mut conn = server.db_conn();
    assert_eq!(
        thresholds(&mut conn, project_id),
        vec![("*".to_owned(), "value".to_owned(), true)],
        "a name and a UUID of one measure address one threshold"
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

    let project_id = project_id(&server, &fixture);
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

    let project_id = project_id(&server, &fixture);
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

// A report that declares no version takes its thresholds' shape from the project's version.
#[tokio::test]
async fn an_undeclared_version_takes_the_shape_from_the_project() {
    let server = TestServer::new().await;
    // The first signup is the server admin, so this fixture can move its project's version.
    let fixture = fixture(&server, "inherited").await;
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({ "bmf_version": BmfVersion::V1 }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "PATCH bmf_version");

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)])
            .undeclared()
            .thresholds(serde_json::json!({ "models": { "latency": model() } })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a map: {body}");
    let refusal = serde_json::from_str::<serde_json::Value>(&body).expect("the refusal is JSON");
    assert_eq!(
        refusal["message"],
        "The report is read as BMF version 1, so `thresholds.models` must be a list of threshold entries, but a map was sent.",
        "the refusal names the project's version: {body}"
    );

    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)])
            .undeclared()
            .thresholds(entries()),
    )
    .await;
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
async fn three_thresholds(server: &TestServer, label: &str) -> (Fixture, ProjectId) {
    let fixture = fixture(server, label).await;
    report(
        server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "metric": "value", "model": model() },
                { "measure": "latency", "metric": "p99", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "metric": "value",
                    "model": model(),
                },
            ]
        })),
    )
    .await;
    let project_id = project_id(server, &fixture);
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

// `reset` has one meaning at every version, so a version 0 reset strips the named and
// filtered thresholds that a map has no way to spell.
#[tokio::test]
async fn v0_reset_strips_every_threshold() {
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
            ("*".to_owned(), "p99".to_owned(), false),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), false),
        ],
        "a version 0 reset reaches every threshold on the branch and testbed"
    );
}

// `reset` strips every threshold a version 1 list did not name.
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

// A bare `reset` strips every threshold on the branch and testbed.
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
        "a reset that names nothing strips everything"
    );
}

// A threshold on another branch or testbed is not on this report's, so `reset` does not
// reach it.
#[tokio::test]
async fn v1_reset_stays_on_the_report_branch_and_testbed() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "resetscope").await;

    let bare = serde_json::json!({
        "models": [{ "measure": "latency", "metric": "value", "model": model() }]
    });
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

    let project_id = project_id(&server, &fixture);
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
// `bencher run --thresholds-reset` sends today.
#[tokio::test]
async fn v0_map_with_reset_updates_what_it_names_and_strips_every_other_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0mapreset").await;

    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "metric": "value", "model": model() },
                { "measure": "throughput", "metric": "value", "model": model() },
                { "measure": "latency", "metric": "p99", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "metric": "value",
                    "model": model(),
                },
            ]
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
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
            ("*".to_owned(), "p99".to_owned(), false),
            (r#"[{"size_mb":16}]"#.to_owned(), "value".to_owned(), false),
        ],
        "the map keeps the measure it named and reset strips every threshold it did not"
    );

    let models = schema::model::table
        .filter(schema::model::threshold_id.eq(latency))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("Failed to count the models");
    assert_eq!(models, 2, "the map updated the model it named");
}

// The ceiling counts what a version 1 list creates, so a list whose creates would pass it creates
// none of them, and the report and its results are still created.
#[cfg(feature = "plus")]
#[tokio::test]
async fn a_v1_list_whose_creates_pass_the_ceiling_creates_none() {
    let server = TestServer::new_with_creation_limits(3, 3).await;
    let fixture = fixture(&server, "ceiling").await;

    let created = report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "metric": "value", "model": model() },
                { "measure": "latency", "metric": "p99", "model": model() },
                { "measure": "latency", "metric": "p50", "model": model() },
                {
                    "parameters": [{ "size_mb": 16 }],
                    "measure": "latency",
                    "metric": "value",
                    "model": model(),
                },
            ]
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    assert!(
        thresholds(&mut conn, project_id).is_empty(),
        "the report created no threshold"
    );
    let results = schema::report_benchmark::table
        .inner_join(schema::report::table)
        .filter(schema::report::project_id.eq(project_id))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("Failed to count the results");
    assert_eq!(results, 2, "the report kept a result for each variant");
    assert_eq!(created.warnings, Some(skipped_thresholds(4)));
}

// A report that skips thresholds past the cap says how many when it is created and when it is read.
#[tokio::test]
async fn a_report_past_the_active_cap_warns_of_what_it_skipped() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "capwarn").await;
    let declared = (0..MAX_ACTIVE_THRESHOLDS + 2)
        .map(|n| {
            serde_json::json!({
                "measure": "latency",
                "metric": format!("m-{n}"),
                "model": model(),
            })
        })
        .collect::<Vec<_>>();

    let created = report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({ "models": declared })),
    )
    .await;
    assert_eq!(created.warnings, Some(skipped_thresholds(2)), "POST");
    let read = get_report(&server, &fixture, created.uuid).await;
    assert_eq!(read.warnings, Some(skipped_thresholds(2)), "GET");
}

// A report whose declared thresholds all fit under the cap skipped nothing, so it has no `warnings`
// key.
#[tokio::test]
async fn a_report_whose_thresholds_fit_under_the_active_cap_has_no_warnings() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "fitwarn").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "metric": "first", "model": model() },
                { "measure": "latency", "metric": "second", "model": model() },
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let report: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse the report");
    assert!(report.get("warnings").is_none(), "{body}");
}

// Past the cap on active thresholds for a measure, a report creates what fits in payload order
// and skips the rest. The ceiling sits one past what fits, so checking it with every declared
// create, before the cap, would skip them all.
#[cfg(feature = "plus")]
#[tokio::test]
async fn a_report_past_the_active_cap_creates_what_fits_in_payload_order() {
    let fits = u32::try_from(MAX_ACTIVE_THRESHOLDS).expect("the cap fits a u32");
    let server = TestServer::new_with_creation_limits(fits + 1, fits + 1).await;
    let fixture = fixture(&server, "activecap").await;

    let seeds = (1..MAX_ACTIVE_THRESHOLDS)
        .map(|n| {
            serde_json::json!({
                "measure": "latency",
                "metric": format!("seed-{n}"),
                "model": model(),
            })
        })
        .collect::<Vec<_>>();
    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({ "models": seeds })),
    )
    .await;
    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({
            "models": [
                { "measure": "latency", "metric": "first", "model": model() },
                { "measure": "latency", "metric": "second", "model": model() },
                { "measure": "latency", "metric": "third", "model": model() },
            ]
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    let thresholds = thresholds(&mut conn, project_id);
    assert_eq!(thresholds.len(), MAX_ACTIVE_THRESHOLDS);
    assert_eq!(
        thresholds.last(),
        Some(&("*".to_owned(), "first".to_owned(), true)),
        "the first threshold the payload declared is the one that fits"
    );
}

// A threshold with no model does not count toward the cap, and giving it a model back does, in
// payload order like a create.
#[tokio::test]
async fn a_report_counts_giving_a_model_back_toward_the_active_cap() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "capback").await;
    let seed = |n: usize| {
        serde_json::json!({
            "measure": "latency",
            "metric": format!("seed-{n}"),
            "model": model(),
        })
    };

    let seeds = (1..=MAX_ACTIVE_THRESHOLDS).map(seed).collect::<Vec<_>>();
    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({ "models": seeds })),
    )
    .await;
    let kept = (2..=MAX_ACTIVE_THRESHOLDS).map(seed).collect::<Vec<_>>();
    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)])
            .thresholds(serde_json::json!({ "models": kept, "reset": true })),
    )
    .await;
    report(
        &server,
        &fixture,
        Post::new(3, vec![steady(2)]).thresholds(serde_json::json!({
            "models": [
                seed(1),
                { "measure": "latency", "metric": "past", "model": model() },
            ]
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    let thresholds = thresholds(&mut conn, project_id);
    assert_eq!(thresholds.len(), MAX_ACTIVE_THRESHOLDS, "past was skipped");
    assert_eq!(
        thresholds.first(),
        Some(&("*".to_owned(), "seed-1".to_owned(), true)),
        "the stripped threshold got its model back"
    );
}

// A report at the cap that resets and declares as many different thresholds swaps them all, since
// the thresholds a reset strips make room for what the same report declares.
#[tokio::test]
async fn a_reset_report_at_the_active_cap_swaps_every_threshold() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "capreset").await;
    let named = |prefix: &str| {
        (1..=MAX_ACTIVE_THRESHOLDS)
            .map(|n| {
                serde_json::json!({
                    "measure": "latency",
                    "metric": format!("{prefix}-{n}"),
                    "model": model(),
                })
            })
            .collect::<Vec<_>>()
    };

    report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({ "models": named("old") })),
    )
    .await;
    report(
        &server,
        &fixture,
        Post::new(2, vec![steady(1)]).thresholds(serde_json::json!({
            "models": named("new"),
            "reset": true,
        })),
    )
    .await;

    let project_id = project_id(&server, &fixture);
    let mut conn = server.db_conn();
    let active = thresholds(&mut conn, project_id)
        .into_iter()
        .filter(|(_, _, has_model)| *has_model)
        .collect::<Vec<_>>();
    let new = (1..=MAX_ACTIVE_THRESHOLDS)
        .map(|n| ("*".to_owned(), format!("new-{n}"), true))
        .collect::<Vec<_>>();
    assert_eq!(active, new);
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
            "models": [{ "measure": "latency", "metric": "value", "model": { "test": "bogus" } }]
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
            "models": [{ "metric": "value", "model": { "test": "static" } }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a missing measure: {body}");
    assert!(
        body.contains("thresholds.models[0]") && body.contains("missing field `measure`"),
        "the refusal names the entry and the field it wants: {body}"
    );
}

// A version 1 entry names its metric, even the conventional `value`.
#[tokio::test]
async fn a_v1_entry_without_a_metric_is_a_bad_request() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "nometric").await;

    let (status, body) = try_report(
        &server,
        &fixture,
        Post::new(1, vec![steady(0)]).thresholds(serde_json::json!({
            "models": [{ "measure": "latency", "model": model() }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a missing metric: {body}");
    assert!(
        body.contains("thresholds.models[0]") && body.contains("missing field `metric`"),
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
