#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The correction across lines, end to end through ingest: at BMF version 1, a t-test,
//! z-score, or log-normal boundary covers every check its model makes in one report.
//!
//! Every boundary here leaves a chance that is a power of two, and every family has a
//! power of two members, so each per check level is exact in binary and a corrected
//! limit is compared bit for bit with the limit of a one line report checked at that level.

use bencher_api_tests::{TestOrg, TestServer, TestUser};
use bencher_json::{
    JsonReport, MetricName, ProjectSlug, ReportUuid,
    project::{boundary::BoundaryLimit, console::JsonConsoleAlertDetail},
};
use bencher_schema::{context::DbConnection, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// The history every line reports before the report under test, one value a day.
const HISTORY: [f64; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];
/// What every unchanged line reports in the report under test.
const STEADY: f64 = 12.0;

/// A user with an organization, who makes one project per scenario.
struct Owner {
    user: TestUser,
    org: TestOrg,
}

impl Owner {
    async fn new(server: &TestServer, label: &str) -> Self {
        let user = server
            .signup("Test User", &format!("corr{label}@example.com"))
            .await;
        let org = server.create_org(&user, &format!("Corr Org {label}")).await;
        Self { user, org }
    }

    async fn project(&self, server: &TestServer, label: &str) -> Project {
        let project = server
            .create_project(&self.user, &self.org, &format!("Corr {label}"))
            .await;
        Project {
            slug: project.slug,
            token: self.user.token.clone(),
        }
    }
}

struct Project {
    slug: ProjectSlug,
    token: String,
}

impl Project {
    /// Post one report and require it to be created.
    async fn report(
        &self,
        server: &TestServer,
        day: usize,
        bmf_version: u8,
        results: Vec<String>,
        thresholds: &serde_json::Value,
    ) -> ReportUuid {
        let body = serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": format!("2024-01-{day:02}T00:00:00Z"),
            "end_time": format!("2024-01-{day:02}T00:01:00Z"),
            "results": results,
            "bmf_version": bmf_version,
            "thresholds": thresholds,
        });
        self.post(server, &body).await
    }

    /// Post one report body and require it to be created.
    async fn post(&self, server: &TestServer, body: &serde_json::Value) -> ReportUuid {
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{}/reports", self.slug)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&self.token),
            )
            .json(body)
            .send()
            .await
            .expect("Request failed");
        let status = resp.status();
        let body = resp.text().await.expect("Failed to read the response");
        assert_eq!(status, StatusCode::CREATED, "POST report: {body}");
        serde_json::from_str::<JsonReport>(&body)
            .expect("Failed to parse the report")
            .uuid
    }

    /// Report the history of every line, then the report under test, and return its UUID.
    async fn run(&self, server: &TestServer, scenario: &Scenario<'_>) -> ReportUuid {
        let Scenario {
            bmf_version,
            thresholds,
            lines,
            fresh,
            iterations,
            outlier,
        } = scenario;
        let thresholds = thresholds_payload(*bmf_version, thresholds);
        for (day, value) in HISTORY.into_iter().enumerate() {
            let values = vec![value; *lines];
            self.report(
                server,
                day + 1,
                *bmf_version,
                vec![payload(*bmf_version, &values)],
                &thresholds,
            )
            .await;
        }
        let mut values = vec![STEADY; lines + fresh];
        if let Some((line, value)) = outlier
            && let Some(slot) = values.get_mut(*line)
        {
            *slot = *value;
        }
        let results = vec![payload(*bmf_version, &values); *iterations];
        self.report(
            server,
            HISTORY.len() + 1,
            *bmf_version,
            results,
            &thresholds,
        )
        .await
    }
}

/// One report under test, after the history of its lines.
struct Scenario<'a> {
    bmf_version: u8,
    /// Each threshold as the metric it checks and its model.
    thresholds: &'a [(&'a str, serde_json::Value)],
    /// The lines with a history.
    lines: usize,
    /// The lines the report under test adds, which have no history.
    fresh: usize,
    iterations: usize,
    /// One line that reports this value in place of the steady one.
    outlier: Option<(usize, f64)>,
}

impl<'a> Scenario<'a> {
    fn new(bmf_version: u8, thresholds: &'a [(&'a str, serde_json::Value)], lines: usize) -> Self {
        Self {
            bmf_version,
            thresholds,
            lines,
            fresh: 0,
            iterations: 1,
            outlier: None,
        }
    }
}

fn t_test(lower: Option<f64>, upper: Option<f64>) -> serde_json::Value {
    model("t_test", lower, upper)
}

fn model(test: &str, lower: Option<f64>, upper: Option<f64>) -> serde_json::Value {
    serde_json::json!({
        "test": test,
        "min_sample_size": 2,
        "max_sample_size": 64,
        "lower_boundary": lower,
        "upper_boundary": upper,
    })
}

/// The thresholds a report declares, in the shape its version takes.
fn thresholds_payload(
    bmf_version: u8,
    thresholds: &[(&str, serde_json::Value)],
) -> serde_json::Value {
    if bmf_version == 0 {
        let models = thresholds
            .iter()
            .map(|(_, model)| ("latency".to_owned(), model.clone()))
            .collect::<serde_json::Map<_, _>>();
        serde_json::json!({ "models": models })
    } else {
        let models = thresholds
            .iter()
            .map(|(metric, model)| {
                serde_json::json!({ "measure": "latency", "metric": metric, "model": model })
            })
            .collect::<Vec<_>>();
        serde_json::json!({ "models": models })
    }
}

/// One iteration of results with one line per value, each carrying the value as its point
/// estimate and as its `p99`.
fn payload(bmf_version: u8, values: &[f64]) -> String {
    if bmf_version == 0 {
        let benchmarks = values
            .iter()
            .enumerate()
            .map(|(line, value)| {
                (
                    format!("bench-{line}"),
                    serde_json::json!({ "latency": { "value": value } }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        serde_json::Value::Object(benchmarks).to_string()
    } else {
        let entries = values
            .iter()
            .enumerate()
            .map(|(line, value)| {
                serde_json::json!({
                    "parameters": { "line": line },
                    "measures": { "latency": { "value": value, "p99": value } },
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({ "bench": entries }).to_string()
    }
}

/// One boundary of the report under test.
#[derive(Debug, Clone, PartialEq)]
struct Limits {
    iteration: i32,
    metric: String,
    lower: Option<f64>,
    upper: Option<f64>,
}

/// Every boundary of a report, sorted by iteration and metric name.
fn limits(conn: &mut DbConnection, report: ReportUuid) -> Vec<Limits> {
    let mut limits = schema::boundary::table
        .inner_join(
            schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(schema::report::table)),
        )
        .filter(schema::report::uuid.eq(report.to_string()))
        .select((
            schema::report_benchmark::iteration,
            schema::metric::name,
            schema::boundary::lower_limit,
            schema::boundary::upper_limit,
        ))
        .load::<(i32, MetricName, Option<f64>, Option<f64>)>(conn)
        .expect("Failed to load the boundaries")
        .into_iter()
        .map(|(iteration, metric, lower, upper)| Limits {
            iteration,
            metric: metric.to_string(),
            lower,
            upper,
        })
        .collect::<Vec<_>>();
    limits.sort_by(|a, b| (a.iteration, &a.metric).cmp(&(b.iteration, &b.metric)));
    limits
}

/// How many alerts a report raised.
fn alert_count(conn: &mut DbConnection, report: ReportUuid) -> i64 {
    schema::alert::table
        .inner_join(schema::report::table)
        .filter(schema::report::uuid.eq(report.to_string()))
        .count()
        .get_result(conn)
        .expect("Failed to count the alerts")
}

/// The one boundary of a one line report checked by one threshold, after the same history.
async fn one_line(
    server: &TestServer,
    owner: &Owner,
    label: &str,
    bmf_version: u8,
    model: serde_json::Value,
) -> Limits {
    let project = owner.project(server, label).await;
    let thresholds = [("value", model)];
    let report = project
        .run(server, &Scenario::new(bmf_version, &thresholds, 1))
        .await;
    let mut conn = server.db_conn();
    let mut limits = limits(&mut conn, report);
    assert_eq!(limits.len(), 1, "one line, one threshold: {limits:?}");
    limits.pop().expect("one boundary")
}

/// Every boundary of a report has these limits.
fn assert_every(limits: &[Limits], lower: Option<f64>, upper: Option<f64>, what: &str) {
    assert!(!limits.is_empty(), "{what}: no boundaries");
    for limit in limits {
        assert_eq!(
            (limit.lower, limit.upper),
            (lower, upper),
            "{what}: {limit:?}"
        );
    }
}

// A version 1 report with one line is a family of one, so its limit is the limit a
// version 0 report computes.
#[tokio::test]
async fn a_version_1_report_with_one_line_keeps_its_limit() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "one").await;

    let v0 = one_line(&server, &owner, "v0", 0, t_test(None, Some(0.75))).await;
    let v1 = one_line(&server, &owner, "v1", 1, t_test(None, Some(0.75))).await;
    assert!(v0.upper.is_some(), "the history is long enough to test");
    assert_eq!((v1.lower, v1.upper), (v0.lower, v0.upper));
}

// A version 1 report with 16 lines tests each at 1 - (1 - 0.75) / 16, and an outlier
// that alerts on a report of one line does not alert on a report of 16.
#[tokio::test]
async fn a_version_1_report_spreads_its_boundary_across_its_lines() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "many").await;

    let per_line = one_line(&server, &owner, "level", 1, t_test(None, Some(0.984_375))).await;

    let outlier = 15.0;
    let alone = owner.project(&server, "alone").await;
    let thresholds = [("value", t_test(None, Some(0.75)))];
    let report = alone
        .run(
            &server,
            &Scenario {
                outlier: Some((0, outlier)),
                ..Scenario::new(1, &thresholds, 1)
            },
        )
        .await;
    let mut conn = server.db_conn();
    assert_eq!(
        alert_count(&mut conn, report),
        1,
        "the outlier alerts on its own"
    );
    drop(conn);

    let many = owner.project(&server, "many").await;
    let report = many
        .run(
            &server,
            &Scenario {
                outlier: Some((0, outlier)),
                ..Scenario::new(1, &thresholds, 16)
            },
        )
        .await;
    let mut conn = server.db_conn();
    let limits = limits(&mut conn, report);
    assert_eq!(limits.len(), 16);
    assert_every(&limits, per_line.lower, per_line.upper, "16 lines");
    assert_eq!(
        alert_count(&mut conn, report),
        0,
        "the outlier is within a limit that covers 16 lines"
    );
}

// A z-score and a log-normal boundary are probabilities too, and the log-normal lower
// limit keeps its reflection at the corrected level.
#[tokio::test]
async fn a_z_score_or_log_normal_threshold_is_corrected() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "zln").await;

    for (test, lower) in [("z_score", None), ("log_normal", Some(0.75))] {
        let label = test.replace('_', " ");
        let per_line = one_line(
            &server,
            &owner,
            &format!("{label} level"),
            1,
            model(test, lower.map(|_| 0.984_375), Some(0.984_375)),
        )
        .await;
        assert!(
            per_line.upper.is_some(),
            "{test}: the history is long enough"
        );

        let project = owner.project(&server, &format!("{label} many")).await;
        let thresholds = [("value", model(test, lower, Some(0.75)))];
        let report = project
            .run(&server, &Scenario::new(1, &thresholds, 16))
            .await;
        let mut conn = server.db_conn();
        let limits = limits(&mut conn, report);
        assert_eq!(limits.len(), 16, "{test}");
        assert_every(&limits, per_line.lower, per_line.upper, test);
    }
}

// Each iteration of a line is its own member of the family: 2 lines in 2 iterations are
// a family of 4.
#[tokio::test]
async fn a_version_1_report_counts_each_iteration() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "iter").await;

    let per_line = one_line(&server, &owner, "level", 1, t_test(None, Some(0.9375))).await;

    let project = owner.project(&server, "iterations").await;
    let thresholds = [("value", t_test(None, Some(0.75)))];
    let report = project
        .run(
            &server,
            &Scenario {
                iterations: 2,
                ..Scenario::new(1, &thresholds, 2)
            },
        )
        .await;
    let mut conn = server.db_conn();
    let limits = limits(&mut conn, report);
    assert_eq!(limits.len(), 4, "2 lines in 2 iterations: {limits:?}");
    // The second iteration's history holds the first, so only the first is compared.
    let first = limits
        .into_iter()
        .filter(|limit| limit.iteration == 0)
        .collect::<Vec<_>>();
    assert_eq!(first.len(), 2);
    assert_every(
        &first,
        per_line.lower,
        per_line.upper,
        "the first iteration",
    );
}

// A version 0 report keeps the per line meaning, however many lines it has.
#[tokio::test]
async fn a_version_0_report_keeps_the_per_line_limits() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "v0").await;

    let per_line = one_line(&server, &owner, "one", 0, t_test(None, Some(0.75))).await;

    let project = owner.project(&server, "many").await;
    let thresholds = [("value", t_test(None, Some(0.75)))];
    let report = project
        .run(&server, &Scenario::new(0, &thresholds, 16))
        .await;
    let mut conn = server.db_conn();
    let limits = limits(&mut conn, report);
    assert_eq!(limits.len(), 16);
    assert_every(
        &limits,
        per_line.lower,
        per_line.upper,
        "16 lines at version 0",
    );
}

// A percentage, IQR, or static boundary is a size, not a chance, so it is not spread
// across a version 1 report.
#[tokio::test]
async fn a_percentage_or_iqr_threshold_is_not_corrected() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "size").await;

    for (test, model) in [
        ("percentage", model("percentage", Some(0.25), Some(0.5))),
        ("iqr", model("iqr", Some(1.5), Some(3.0))),
        ("delta_iqr", model("delta_iqr", Some(1.5), Some(3.0))),
        // A static model takes no sample size. A correction would move its lower
        // boundary of 0.5.
        (
            "static",
            serde_json::json!({
                "test": "static",
                "lower_boundary": 0.5,
                "upper_boundary": 100.0,
            }),
        ),
    ] {
        let label = test.replace('_', " ");
        let per_line = one_line(&server, &owner, &format!("{label} one"), 1, model.clone()).await;
        assert!(
            per_line.upper.is_some(),
            "{test}: the history is long enough"
        );

        let project = owner.project(&server, &format!("{label} many")).await;
        let thresholds = [("value", model)];
        let report = project
            .run(&server, &Scenario::new(1, &thresholds, 16))
            .await;
        let mut conn = server.db_conn();
        let limits = limits(&mut conn, report);
        assert_eq!(limits.len(), 16, "{test}");
        assert_every(&limits, per_line.lower, per_line.upper, test);
    }
}

// Two corrected thresholds on one report are one family of 8, and each spends its own
// boundary's share: 1 - (1 - 0.75) / 8 for one and 1 - (1 - 0.875) / 8 for the other.
#[tokio::test]
async fn each_threshold_spends_its_own_boundary() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "mixed").await;

    let value_line = one_line(&server, &owner, "value", 1, t_test(None, Some(0.968_75))).await;
    let p99_line = one_line(&server, &owner, "p99", 1, t_test(None, Some(0.984_375))).await;

    let project = owner.project(&server, "both").await;
    let thresholds = [
        ("value", t_test(None, Some(0.75))),
        ("p99", t_test(None, Some(0.875))),
    ];
    let report = project
        .run(&server, &Scenario::new(1, &thresholds, 4))
        .await;
    let mut conn = server.db_conn();
    let (value, p99): (Vec<_>, Vec<_>) = limits(&mut conn, report)
        .into_iter()
        .partition(|limit| limit.metric == "value");
    assert_eq!((value.len(), p99.len()), (4, 4));
    assert_every(&value, value_line.lower, value_line.upper, "value");
    assert_every(&p99, p99_line.lower, p99_line.upper, "p99");
}

// A line too new to test still counts: 2 lines with a history and 2 without are a
// family of 4.
#[tokio::test]
async fn a_line_with_too_little_history_still_counts() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "fresh").await;

    let per_line = one_line(&server, &owner, "level", 1, t_test(None, Some(0.9375))).await;

    let project = owner.project(&server, "fresh").await;
    let thresholds = [("value", t_test(None, Some(0.75)))];
    let report = project
        .run(
            &server,
            &Scenario {
                fresh: 2,
                ..Scenario::new(1, &thresholds, 2)
            },
        )
        .await;
    let mut conn = server.db_conn();
    let limits = limits(&mut conn, report);
    assert_eq!(limits.len(), 4, "every line has a boundary row: {limits:?}");
    let (tested, untested): (Vec<_>, Vec<_>) =
        limits.into_iter().partition(|limit| limit.upper.is_some());
    assert_eq!(untested.len(), 2, "the new lines have no limit");
    assert_every(
        &tested,
        per_line.lower,
        per_line.upper,
        "the lines with a history",
    );
    for (_, lower, upper) in levels(&mut conn, report) {
        assert_eq!(
            (lower, upper),
            (None, Some(0.9375)),
            "every line spends its share, tested or not"
        );
    }
}

/// The level each side of every boundary of a report was tested at, by metric name.
fn levels(conn: &mut DbConnection, report: ReportUuid) -> Vec<(String, Option<f64>, Option<f64>)> {
    let mut levels = schema::boundary::table
        .inner_join(
            schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(schema::report::table)),
        )
        .filter(schema::report::uuid.eq(report.to_string()))
        .select((
            schema::metric::name,
            schema::boundary::lower_level,
            schema::boundary::upper_level,
        ))
        .load::<(MetricName, Option<f64>, Option<f64>)>(conn)
        .expect("Failed to load the levels")
        .into_iter()
        .map(|(metric, lower, upper)| (metric.to_string(), lower, upper))
        .collect::<Vec<_>>();
    levels.sort_by(|a, b| a.0.cmp(&b.0));
    levels
}

// The boundary row records the level each corrected side was tested at, counted per
// side: the value threshold guards both sides and the p99 threshold only the upper, so
// 2 lines are a lower family of 2 and an upper family of 4. A version 0 report and a
// percentage threshold record none. The alert detail shows the level of its side.
#[tokio::test]
async fn the_boundary_row_records_the_level_it_tested_at() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "level").await;

    let project = owner.project(&server, "corrected").await;
    let thresholds = [
        ("value", t_test(Some(0.75), Some(0.75))),
        ("p99", t_test(None, Some(0.875))),
    ];
    let report = project
        .run(
            &server,
            &Scenario {
                outlier: Some((0, 1_000.0)),
                ..Scenario::new(1, &thresholds, 2)
            },
        )
        .await;
    let mut conn = server.db_conn();
    assert_eq!(
        levels(&mut conn, report),
        vec![
            ("p99".to_owned(), None, Some(0.968_75)),
            ("p99".to_owned(), None, Some(0.968_75)),
            ("value".to_owned(), Some(0.875), Some(0.9375)),
            ("value".to_owned(), Some(0.875), Some(0.9375)),
        ]
    );
    let alert = schema::alert::table
        .inner_join(schema::report::table)
        .inner_join(schema::boundary::table.inner_join(schema::metric::table))
        .filter(schema::report::uuid.eq(report.to_string()))
        .filter(schema::metric::name.eq("value"))
        .select(schema::alert::uuid)
        .first::<String>(&mut conn)
        .expect("The outlier alerts on its point estimate");
    drop(conn);
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/console/projects/{}/alerts/{alert}",
            project.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&project.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let detail: JsonConsoleAlertDetail = resp.json().await.expect("Failed to parse the alert");
    assert_eq!(detail.limit, BoundaryLimit::Upper);
    assert_eq!(
        (
            detail.lower_level.map(f64::from),
            detail.upper_level.map(f64::from)
        ),
        (Some(0.875), Some(0.9375))
    );

    for (label, bmf_version, model) in [
        ("version 0", 0, t_test(None, Some(0.75))),
        ("percentage", 1, model("percentage", None, Some(0.5))),
    ] {
        let project = owner.project(&server, label).await;
        let thresholds = [("value", model)];
        let report = project
            .run(&server, &Scenario::new(bmf_version, &thresholds, 2))
            .await;
        let mut conn = server.db_conn();
        let levels = levels(&mut conn, report);
        assert_eq!(levels.len(), 2, "{label}");
        for (_, lower, upper) in levels {
            assert_eq!((lower, upper), (None, None), "{label}");
        }
    }
}

// Only the checks a corrected model makes join its family. On 4 lines, a t-test on
// `value` makes 4, a t-test on `p99` makes 3 as one line reports no `p99`, and a t-test
// filtered to one line makes 1: a family of 8. A percentage threshold makes no check of
// the family, and neither does a t-test on a measure the report drops.
#[tokio::test]
async fn the_family_counts_only_the_checks_of_its_models() {
    let server = TestServer::new().await;
    let owner = Owner::new(&server, "filters").await;
    let project = owner.project(&server, "filters").await;

    let entries = (0..4)
        .map(|line| {
            let mut latency = serde_json::json!({ "value": STEADY, "p50": STEADY });
            if line < 3 {
                latency["p99"] = serde_json::json!(STEADY);
            }
            serde_json::json!({
                "parameters": { "line": line },
                "measures": {
                    "latency": latency,
                    "throughput": { "value": STEADY },
                },
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [serde_json::json!({ "bench": entries }).to_string()],
        "bmf_version": 1,
        "settings": { "measures": ["latency"] },
        "thresholds": {
            "models": [
                { "measure": "latency", "metric": "value", "model": t_test(None, Some(0.75)) },
                {
                    "measure": "latency",
                    "metric": "p50",
                    "model": model("percentage", None, Some(0.5)),
                },
                { "measure": "latency", "metric": "p99", "model": t_test(None, Some(0.75)) },
                {
                    "parameters": [{ "line": 0 }],
                    "measure": "latency",
                    "metric": "value",
                    "model": t_test(None, Some(0.75)),
                },
                {
                    "measure": "throughput",
                    "metric": "value",
                    "model": t_test(None, Some(0.75)),
                },
            ]
        },
    });
    let report = project.post(&server, &body).await;

    let mut conn = server.db_conn();
    let corrected = (None, Some(0.968_75));
    let expected = std::iter::repeat_n(("p50", (None, None)), 4)
        .chain(std::iter::repeat_n(("p99", corrected), 3))
        .chain(std::iter::repeat_n(("value", corrected), 5))
        .map(|(metric, (lower, upper))| (metric.to_owned(), lower, upper))
        .collect::<Vec<_>>();
    assert_eq!(levels(&mut conn, report), expected);
}
