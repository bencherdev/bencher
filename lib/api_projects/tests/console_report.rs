#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The console's report lines: `/v0/projects/{project}/console/reports/{report}`.
//!
//! Every report here is posted through the API with a fixed time, so the
//! thresholds compute real boundaries and alerts from a known history.

use bencher_api_tests::TestServer;
use bencher_json::{
    DateTime, DateTimeMillis, JsonReport, MeasureUuid, ModelTest, ProjectSlug, ProjectUuid,
    ReportUuid, VersionUuid,
    project::{
        Visibility,
        alert::AlertStatus,
        boundary::BoundaryLimit,
        console::{JsonConsoleReport, JsonConsoleReportLine, MAX_CONSOLE_HISTORY_REPORTS},
        head::VersionNumber,
        report::Adapter,
    },
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// The server's frozen clock, a few days after the last report.
const NOW: &str = "2024-03-01T00:00:00Z";

fn at(time: &str) -> DateTime {
    serde_json::from_value(serde_json::json!(time)).expect("Failed to parse the time")
}

fn millis(time: &str) -> i64 {
    DateTimeMillis::from(at(time)).into()
}

struct Fixture {
    slug: ProjectSlug,
    uuid: ProjectUuid,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("console{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Console Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Console Project {label}"))
        .await;
    Fixture {
        slug: project.slug,
        uuid: project.uuid,
        token: user.token,
    }
}

/// The values one report carries, one per line.
#[derive(Clone, Copy)]
struct Values {
    alpha_2_latency: f64,
    alpha_10_latency: f64,
    alpha_2_throughput: f64,
    alpha_10_throughput: f64,
    beta_latency: f64,
    beta_p99: f64,
    gamma_latency: f64,
}

const STEADY: Values = Values {
    alpha_2_latency: 100.0,
    alpha_10_latency: 200.0,
    alpha_2_throughput: 50.0,
    alpha_10_throughput: 50.0,
    beta_latency: 10.0,
    beta_p99: 30.0,
    gamma_latency: 5.0,
};

/// Against a steady history: `alpha` at size 10 and `gamma` cross their upper
/// limits, `alpha` at size 2 creeps up, its throughput falls toward its lower
/// limit, `alpha` at size 10 gains throughput, and `beta`'s `p99` has no threshold.
const SUBJECT: Values = Values {
    alpha_2_latency: 103.0,
    alpha_10_latency: 260.0,
    alpha_2_throughput: 49.0,
    alpha_10_throughput: 55.0,
    beta_latency: 10.5,
    beta_p99: 40.0,
    gamma_latency: 7.0,
};

const ELSEWHERE: Values = Values {
    alpha_2_latency: 999.0,
    alpha_10_latency: 999.0,
    alpha_2_throughput: 999.0,
    alpha_10_throughput: 999.0,
    beta_latency: 999.0,
    beta_p99: 999.0,
    gamma_latency: 999.0,
};

fn results(values: Values) -> String {
    let Values {
        alpha_2_latency,
        alpha_10_latency,
        alpha_2_throughput,
        alpha_10_throughput,
        beta_latency,
        beta_p99,
        gamma_latency,
    } = values;
    serde_json::json!({
        "alpha": [
            {
                "parameters": { "size": 2 },
                "measures": {
                    "latency": { "value": alpha_2_latency },
                    "throughput": { "value": alpha_2_throughput },
                },
            },
            {
                "parameters": { "size": 10 },
                "measures": {
                    "latency": { "value": alpha_10_latency },
                    "throughput": { "value": alpha_10_throughput },
                },
            },
        ],
        "beta": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": beta_latency, "p99": beta_p99 } },
        }],
        "gamma": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": gamma_latency } },
        }],
    })
    .to_string()
}

/// Latency guards its upper side and throughput its lower side, each a quarter
/// off the mean of the history so every limit is exact.
fn thresholds() -> serde_json::Value {
    serde_json::json!({
        "models": [
            {
                "measure": "latency",
                "metric": "value",
                "model": { "test": "percentage", "upper_boundary": 0.25 },
            },
            {
                "measure": "throughput",
                "metric": "value",
                "model": { "test": "percentage", "lower_boundary": 0.25 },
            },
        ]
    })
}

struct Post {
    time: &'static str,
    branch: &'static str,
    testbed: &'static str,
    hash: &'static str,
    iterations: Vec<String>,
    thresholds: Option<serde_json::Value>,
    seconds: i64,
}

impl Post {
    fn main(time: &'static str, hash: &'static str, values: Values) -> Self {
        Self {
            time,
            branch: "main",
            testbed: "localhost",
            hash,
            iterations: vec![results(values)],
            thresholds: Some(thresholds()),
            seconds: 60,
        }
    }
}

async fn post(server: &TestServer, fixture: &Fixture, post: Post) -> JsonReport {
    let Post {
        time,
        branch,
        testbed,
        hash,
        iterations,
        thresholds,
        seconds,
    } = post;
    let start_time = at(time);
    let end_time =
        DateTime::try_from(start_time.timestamp() + seconds).expect("Failed to end the run");
    let body = serde_json::json!({
        "branch": branch,
        "testbed": testbed,
        "hash": hash,
        "start_time": start_time,
        "end_time": end_time,
        "results": iterations,
        "bmf_version": 1,
        "thresholds": thresholds,
        "settings": { "adapter": "json" },
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read the response");
    assert_eq!(status, StatusCode::CREATED, "POST report at {time}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

/// The reports of the shared history, in the order they were posted.
struct Seeded {
    fixture: Fixture,
    first: JsonReport,
    jan_20: JsonReport,
    feb_01: JsonReport,
    feb_10: JsonReport,
    feb_20: JsonReport,
    subject: JsonReport,
    next: JsonReport,
}

/// A steady history on `main` and `localhost`, interleaved with a report on
/// another branch and one on another testbed, then the subject report and one
/// after it.
async fn seeded(server: &TestServer, label: &str) -> Seeded {
    let fixture = fixture(server, label).await;
    let first = post(
        server,
        &fixture,
        Post::main(
            "2023-11-01T00:00:00Z",
            "1000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    let jan_20 = post(
        server,
        &fixture,
        Post::main(
            "2024-01-20T00:00:00Z",
            "1000000000000000000000000000000000000002",
            STEADY,
        ),
    )
    .await;
    // A run that straddles the default window's start, which a start before the
    // window keeps out.
    post(
        server,
        &fixture,
        Post::main(
            "2024-01-27T23:59:30Z",
            "100000000000000000000000000000000000000a",
            STEADY,
        ),
    )
    .await;
    let feb_01 = post(
        server,
        &fixture,
        Post::main(
            "2024-02-01T00:00:00Z",
            "1000000000000000000000000000000000000003",
            STEADY,
        ),
    )
    .await;
    let feb_10 = post(
        server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:00Z",
            "1000000000000000000000000000000000000004",
            STEADY,
        ),
    )
    .await;
    let feb_20 = post(
        server,
        &fixture,
        Post::main(
            "2024-02-20T00:00:00Z",
            "1000000000000000000000000000000000000005",
            STEADY,
        ),
    )
    .await;
    post(
        server,
        &fixture,
        Post {
            branch: "feature",
            thresholds: None,
            ..Post::main(
                "2024-02-22T00:00:00Z",
                "1000000000000000000000000000000000000006",
                ELSEWHERE,
            )
        },
    )
    .await;
    post(
        server,
        &fixture,
        Post {
            testbed: "elsewhere",
            thresholds: None,
            ..Post::main(
                "2024-02-23T00:00:00Z",
                "1000000000000000000000000000000000000007",
                ELSEWHERE,
            )
        },
    )
    .await;
    let subject = post(
        server,
        &fixture,
        Post::main(
            "2024-02-25T00:00:00Z",
            "1000000000000000000000000000000000000008",
            SUBJECT,
        ),
    )
    .await;
    let next = post(
        server,
        &fixture,
        Post::main(
            "2024-02-27T00:00:00Z",
            "1000000000000000000000000000000000000009",
            STEADY,
        ),
    )
    .await;
    Seeded {
        fixture,
        first,
        jan_20,
        feb_01,
        feb_10,
        feb_20,
        subject,
        next,
    }
}

async fn try_console_report(
    server: &TestServer,
    project: &ProjectSlug,
    report: &JsonReport,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, String) {
    let mut request = server.client.get(server.api_url(&format!(
        "/v0/projects/{project}/console/reports/{}{query}",
        report.uuid
    )));
    if let Some(token) = token {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        );
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    (
        status,
        resp.text().await.expect("Failed to read the response"),
    )
}

async fn console_report(
    server: &TestServer,
    fixture: &Fixture,
    report: &JsonReport,
    query: &str,
) -> JsonConsoleReport {
    let (status, text) =
        try_console_report(server, &fixture.slug, report, query, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "GET console report: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console report")
}

/// A line as its row names it: benchmark, variant parameters, measure, metric.
fn name(report: &JsonConsoleReport, line: &JsonConsoleReportLine) -> String {
    format!(
        "{} {} {} {}",
        report.benchmarks[line.benchmark as usize].name,
        report.variants[line.variant as usize].parameters,
        report.measures[line.measure as usize].name,
        line.metric,
    )
}

fn names(report: &JsonConsoleReport) -> Vec<String> {
    report.lines.iter().map(|line| name(report, line)).collect()
}

fn line<'r>(report: &'r JsonConsoleReport, name_of: &str) -> &'r JsonConsoleReportLine {
    report
        .lines
        .iter()
        .find(|line| name(report, line) == name_of)
        .expect("the report has the line")
}

/// Each group as its header names it, with its line, variant, and alert totals.
fn groups(report: &JsonConsoleReport, by_measure: bool) -> Vec<(String, u32, u32, u32)> {
    report
        .groups
        .iter()
        .map(|group| {
            let key = group.key as usize;
            let title = if by_measure {
                report.measures[key].name.to_string()
            } else {
                report.benchmarks[key].name.to_string()
            };
            (title, group.lines, group.variants, group.alerts)
        })
        .collect()
}

fn x(report: &JsonConsoleReport) -> Vec<i64> {
    report.points.x.iter().copied().map(i64::from).collect()
}

const ALPHA_10_LATENCY: &str = r#"alpha {"size":10} Latency value"#;
const BETA_P99: &str = r#"beta {"size":1} Latency p99"#;

// Kills: neighbors read without the branch or testbed filter, or with the order
// reversed, and counts taken from the page instead of the report.
#[tokio::test]
async fn console_report_identity_and_neighbors() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "identity").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "?per_page=1").await;

    assert_eq!(report.uuid, seeded.subject.uuid);
    assert_eq!(report.branch.uuid, seeded.subject.branch.uuid);
    assert_eq!(report.branch.name.as_ref(), "main");
    assert_eq!(report.branch.slug.to_string(), "main");
    assert_eq!(report.testbed.uuid, seeded.subject.testbed.uuid);
    assert_eq!(report.testbed.name.as_ref(), "localhost");
    let version = seeded
        .subject
        .branch
        .head
        .version
        .clone()
        .expect("the subject has a version");
    assert_eq!(report.version.number, version.number);
    assert_eq!(
        report.version.hash.as_ref().map(AsRef::as_ref),
        Some("1000000000000000000000000000000000000008")
    );
    assert_eq!(i64::from(report.start_time), millis("2024-02-25T00:00:00Z"));
    assert_eq!(i64::from(report.end_time), millis("2024-02-25T00:01:00Z"));
    assert!(matches!(report.adapter, Adapter::Json));

    assert_eq!(report.counts.benchmarks, 3);
    assert_eq!(report.counts.variants, 4);
    assert_eq!(report.counts.measures, 2);
    assert_eq!(report.counts.alerts.total, 2);
    assert_eq!(report.counts.alerts.active, 2);
    assert_eq!(report.total, 7);
    assert_eq!(report.lines.len(), 1);

    let previous = report.previous.expect("the subject has a previous report");
    assert_eq!(previous.uuid, seeded.feb_20.uuid);
    assert_eq!(
        i64::from(previous.start_time),
        millis("2024-02-20T00:00:00Z")
    );
    assert_eq!(
        previous.hash.as_ref().map(AsRef::as_ref),
        Some("1000000000000000000000000000000000000005")
    );
    assert!(matches!(previous.adapter, Adapter::Json));
    let next = report.next.expect("the subject has a next report");
    assert_eq!(next.uuid, seeded.next.uuid);
    assert_eq!(i64::from(next.start_time), millis("2024-02-27T00:00:00Z"));

    let first = console_report(&server, &seeded.fixture, &seeded.first, "?per_page=1").await;
    assert!(first.previous.is_none(), "the first report has no previous");
    assert_eq!(
        first.next.map(|next| next.uuid),
        Some(seeded.jan_20.uuid),
        "the report on another branch or testbed is never a neighbor"
    );
}

// Kills: groups ordered by name alone, alerting lines not raised inside their
// group, and parameters compared as text (where `10` sorts before `2`).
#[tokio::test]
async fn console_report_lines_in_drawing_order() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "order").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "").await;

    assert_eq!(
        names(&report),
        vec![
            r#"alpha {"size":10} Latency value"#,
            r#"alpha {"size":2} Latency value"#,
            r#"alpha {"size":2} Throughput value"#,
            r#"alpha {"size":10} Throughput value"#,
            r#"gamma {"size":1} Latency value"#,
            r#"beta {"size":1} Latency p99"#,
            r#"beta {"size":1} Latency value"#,
        ]
    );
    assert_eq!(report.total, 7);
    assert_eq!(
        groups(&report, false),
        vec![
            ("alpha".to_owned(), 4, 2, 1),
            ("gamma".to_owned(), 1, 1, 1),
            ("beta".to_owned(), 2, 1, 0),
        ]
    );
}

// Kills: the group parameter ignored, or measure groups keyed by benchmark.
#[tokio::test]
async fn console_report_lines_grouped_by_measure() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "measure").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "?group=measure").await;

    assert_eq!(
        names(&report),
        vec![
            r#"alpha {"size":10} Latency value"#,
            r#"gamma {"size":1} Latency value"#,
            r#"alpha {"size":2} Latency value"#,
            r#"beta {"size":1} Latency p99"#,
            r#"beta {"size":1} Latency value"#,
            r#"alpha {"size":2} Throughput value"#,
            r#"alpha {"size":10} Throughput value"#,
        ]
    );
    assert_eq!(
        groups(&report, true),
        vec![
            ("Latency".to_owned(), 5, 4, 2),
            ("Throughput".to_owned(), 2, 2, 0),
        ]
    );
}

// Kills: delta read as the raw change rather than toward the guarded side (the
// throughput that fell would sort below the one that rose), groups left in name
// order, and lines no threshold checks sorted anywhere but last.
#[tokio::test]
async fn console_report_lines_sorted_by_delta() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "delta").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "?sort=delta").await;

    assert_eq!(
        names(&report),
        vec![
            r#"gamma {"size":1} Latency value"#,
            r#"alpha {"size":10} Latency value"#,
            r#"alpha {"size":2} Latency value"#,
            r#"alpha {"size":2} Throughput value"#,
            r#"alpha {"size":10} Throughput value"#,
            r#"beta {"size":1} Latency value"#,
            r#"beta {"size":1} Latency p99"#,
        ]
    );
}

// Kills: a page cut before the lines are ordered, a total that counts the page,
// and groups that only cover the page.
#[tokio::test]
async fn console_report_paging_cuts_after_ordering() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "paging").await;

    let second = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?per_page=3&page=2",
    )
    .await;
    assert_eq!(
        names(&second),
        vec![
            r#"alpha {"size":10} Throughput value"#,
            r#"gamma {"size":1} Latency value"#,
            r#"beta {"size":1} Latency p99"#,
        ]
    );
    assert_eq!(second.total, 7);
    assert!(
        second.groups.is_empty(),
        "the client holds the first page's groups"
    );
    assert_eq!(
        second
            .benchmarks
            .iter()
            .map(|benchmark| benchmark.name.to_string())
            .collect::<Vec<_>>(),
        vec!["alpha", "gamma", "beta"],
        "only what the page's lines refer to"
    );

    let first = console_report(&server, &seeded.fixture, &seeded.subject, "?per_page=3").await;
    assert_eq!(groups(&first, false).len(), 3);
    assert_eq!(
        first.benchmarks.len(),
        3,
        "every group's benchmark, for its header"
    );

    let third = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?per_page=3&page=3",
    )
    .await;
    assert_eq!(names(&third), vec![r#"beta {"size":1} Latency value"#]);
    assert_eq!(third.benchmarks.len(), 1);
    assert_eq!(third.measures.len(), 1);

    let past = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?per_page=3&page=4",
    )
    .await;
    assert!(past.lines.is_empty());
    assert_eq!(past.total, 7);

    // A page that no threshold checks still has its history.
    let unchecked = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?per_page=1&page=6",
    )
    .await;
    assert_eq!(names(&unchecked), vec![BETA_P99]);
    let history = &unchecked.lines[0].history;
    assert_eq!(
        history.y,
        vec![Some(30.0), Some(30.0), Some(30.0), Some(40.0)]
    );
    assert!(history.baseline.is_none());
    assert!(history.lower.is_none());
    assert!(history.upper.is_none());
    assert!(unchecked.models.is_empty());
}

// Kills: the threshold, model, baseline, limit, or alert of the row taken from
// anything but the boundary that checked this report's value.
#[tokio::test]
async fn console_report_line_threshold_and_alert() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "threshold").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "").await;

    let alerting = line(&report, ALPHA_10_LATENCY);
    assert!((alerting.value - 260.0).abs() < f64::EPSILON);
    let model = &report.models[alerting.model.expect("a threshold checked it") as usize];
    let alert = seeded
        .subject
        .alerts
        .as_ref()
        .expect("the report has alerts")
        .iter()
        .find(|alert| alert.benchmark.name.as_ref() == "alpha")
        .expect("alpha alerted");
    assert_eq!(model.threshold, alert.threshold.uuid);
    assert_eq!(
        Some(model.uuid),
        alert.threshold.model.as_ref().map(|model| model.uuid)
    );
    assert!(matches!(model.test, ModelTest::Percentage));
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.25));
    assert!(model.lower_boundary.is_none());
    assert_eq!(alerting.baseline, Some(200.0));
    assert_eq!(alerting.upper_limit, Some(250.0));
    assert_eq!(alerting.lower_limit, None);
    let row_alert = alerting.alert.expect("the row alerted");
    assert_eq!(row_alert.uuid, alert.uuid);
    assert!(matches!(row_alert.limit, BoundaryLimit::Upper));
    assert!(matches!(row_alert.status, AlertStatus::Active));

    let unchecked = line(&report, BETA_P99);
    assert!((unchecked.value - 40.0).abs() < f64::EPSILON);
    assert!(unchecked.model.is_none());
    assert!(unchecked.baseline.is_none());
    assert!(unchecked.upper_limit.is_none());
    assert!(unchecked.alert.is_none());

    assert_eq!(
        report.models.len(),
        2,
        "the two thresholds each appear once"
    );
}

// Kills: a window that does not end at the report, history read from another
// branch or testbed, and values misaligned with the shared x.
#[tokio::test]
async fn console_report_history_over_the_default_window() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "history").await;

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "").await;

    assert_eq!(
        i64::from(report.window.start_time),
        millis("2024-01-28T00:00:00Z"),
        "four weeks before the report"
    );
    assert_eq!(
        i64::from(report.window.end_time),
        millis("2024-02-25T00:01:00Z"),
        "the window ends with the report"
    );
    assert!(!report.window.clamped);
    assert_eq!(
        x(&report),
        vec![
            millis("2024-02-01T00:00:00Z"),
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
            millis("2024-02-25T00:00:00Z"),
        ]
    );
    assert_eq!(report.points.report, vec![0, 1, 2, 3]);
    assert!(report.points.iteration.is_none());
    let expected = [
        &seeded.feb_01,
        &seeded.feb_10,
        &seeded.feb_20,
        &seeded.subject,
    ];
    assert_eq!(
        report
            .reports
            .iter()
            .map(|report| report.uuid)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|report| report.uuid)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        report
            .reports
            .iter()
            .map(|report| report.version)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|report| report
                .branch
                .head
                .version
                .as_ref()
                .expect("a version")
                .number)
            .collect::<Vec<VersionNumber>>()
    );

    let alerting = &line(&report, ALPHA_10_LATENCY).history;
    assert_eq!(
        alerting.y,
        vec![Some(200.0), Some(200.0), Some(200.0), Some(260.0)]
    );
    assert_eq!(alerting.baseline, Some(vec![Some(200.0); 4]));
    assert_eq!(alerting.upper, Some(vec![Some(250.0); 4]));
    assert!(
        alerting.lower.is_none(),
        "latency guards its upper side only"
    );
    assert_eq!(alerting.alerts.len(), 1);
    assert_eq!(alerting.alerts[0].index, 3);
    assert_eq!(
        Some(alerting.alerts[0].uuid),
        line(&report, ALPHA_10_LATENCY)
            .alert
            .map(|alert| alert.uuid)
    );

    let unchecked = &line(&report, BETA_P99).history;
    assert_eq!(
        unchecked.y,
        vec![Some(30.0), Some(30.0), Some(30.0), Some(40.0)]
    );
    assert!(unchecked.baseline.is_none());
    assert!(unchecked.upper.is_none());
    assert!(unchecked.lower.is_none());
    assert!(unchecked.alerts.is_empty());
}

// Kills: the start time ignored, and a point whose boundary has no limits yet
// dropped instead of drawn as a gap in the limit column.
#[tokio::test]
async fn console_report_history_from_the_start_time() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "start").await;

    let later = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        &format!("?start_time={}", millis("2024-02-05T00:00:00Z")),
    )
    .await;
    assert_eq!(
        x(&later),
        vec![
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
            millis("2024-02-25T00:00:00Z"),
        ]
    );

    let all = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        &format!("?start_time={}", millis("2023-10-01T00:00:00Z")),
    )
    .await;
    assert!(
        !all.window.clamped,
        "an authenticated window is never clamped"
    );
    assert_eq!(all.reports[0].uuid, seeded.first.uuid);
    assert_eq!(all.reports[1].uuid, seeded.jan_20.uuid);
    let history = &line(&all, ALPHA_10_LATENCY).history;
    assert_eq!(history.y.len(), 7);
    assert_eq!(
        history.upper,
        Some(vec![
            None,
            Some(250.0),
            Some(250.0),
            Some(250.0),
            Some(250.0),
            Some(250.0),
            Some(250.0)
        ]),
        "the first report had no history to set a limit from"
    );
}

// Kills: a window that starts after its own report, which would leave the
// report out of its history.
#[tokio::test]
async fn console_report_window_never_starts_after_the_report() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "after").await;

    for start in ["2024-02-26T00:00:00Z", "2024-02-25T00:00:30Z"] {
        let report = console_report(
            &server,
            &seeded.fixture,
            &seeded.subject,
            &format!("?start_time={}", millis(start)),
        )
        .await;
        assert_eq!(
            i64::from(report.window.start_time),
            millis("2024-02-25T00:00:00Z"),
            "a start at {start} moves back to the report"
        );
        assert_eq!(x(&report), vec![millis("2024-02-25T00:00:00Z")]);
    }
}

/// A key for the fixture's project.
async fn project_key(server: &TestServer, fixture: &Fixture) -> String {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({ "name": "ci" }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read the response");
    assert!(status.is_success(), "POST key: {status} {text}");
    serde_json::from_str::<serde_json::Value>(&text).expect("Failed to parse the key")["key"]
        .as_str()
        .expect("the key")
        .to_owned()
}

fn set_visibility(server: &TestServer, fixture: &Fixture, visibility: Visibility) {
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(fixture.uuid)))
        .set(schema::project::visibility.eq(visibility))
        .execute(&mut conn)
        .expect("Failed to set the visibility");
}

// Kills: a report page served without a login, a signed-in reader turned away
// from a public project, and a project key read on another project.
#[tokio::test]
async fn console_report_needs_a_signed_in_reader() {
    let server = TestServer::new_at(at(NOW)).await;
    let mine = fixture(&server, "keymine").await;
    let theirs = fixture(&server, "keytheirs").await;
    let outsider = server.signup("Outsider", "consolereader@example.com").await;
    let report = post(
        &server,
        &mine,
        Post::main(
            "2024-02-01T00:00:00Z",
            "9000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;

    let (status, _) = try_console_report(&server, &mine.slug, &report, "", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a public project still needs a login"
    );
    let (status, text) =
        try_console_report(&server, &mine.slug, &report, "", Some(&outsider.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");

    set_visibility(&server, &mine, Visibility::Private);
    let mine_key = project_key(&server, &mine).await;
    let theirs_key = project_key(&server, &theirs).await;
    let (status, text) =
        try_console_report(&server, &mine.slug, &report, "", Some(&mine_key)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let (status, _) = try_console_report(&server, &mine.slug, &report, "", Some(&theirs_key)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Kills: a boundary whose limits are not set yet read as no threshold at all.
#[tokio::test]
async fn console_report_null_limits_keep_the_model() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "nulls").await;

    let report = console_report(&server, &seeded.fixture, &seeded.first, "").await;

    let first = line(&report, ALPHA_10_LATENCY);
    assert!(first.model.is_some(), "the threshold checked it");
    assert!(first.baseline.is_none());
    assert!(first.upper_limit.is_none());
    assert!(first.alert.is_none());
    assert_eq!(report.models.len(), 2);
}

// Kills: the row's threshold picked by creation order alone when a later
// threshold raised the alert, and history limits taken from the other one.
#[tokio::test]
async fn console_report_alerted_threshold_wins() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "two").await;
    // The bare threshold comes first and is looser than the filtered one, which
    // only joins it on the third report.
    let bare = serde_json::json!({
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.5 },
    });
    let filtered = serde_json::json!({
        "parameters": [{ "size": 10 }],
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.25 },
    });
    let one = serde_json::json!({ "models": [bare.clone()] });
    let two = serde_json::json!({ "models": [bare, filtered] });
    for (time, hash, thresholds) in [
        (
            "2024-01-20T00:00:00Z",
            "2000000000000000000000000000000000000001",
            &one,
        ),
        (
            "2024-02-01T00:00:00Z",
            "2000000000000000000000000000000000000002",
            &one,
        ),
        (
            "2024-02-10T00:00:00Z",
            "2000000000000000000000000000000000000003",
            &two,
        ),
    ] {
        post(
            &server,
            &fixture,
            Post {
                thresholds: Some(thresholds.clone()),
                ..Post::main(time, hash, STEADY)
            },
        )
        .await;
    }
    let subject = post(
        &server,
        &fixture,
        Post {
            thresholds: Some(two.clone()),
            ..Post::main(
                "2024-02-20T00:00:00Z",
                "2000000000000000000000000000000000000004",
                SUBJECT,
            )
        },
    )
    .await;

    let report = console_report(&server, &fixture, &subject, "").await;

    let alerting = line(&report, ALPHA_10_LATENCY);
    assert_eq!(alerting.upper_limit, Some(250.0));
    assert!(alerting.alert.is_some());
    let model = &report.models[alerting.model.expect("checked") as usize];
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.25));
    assert_eq!(
        alerting.history.upper,
        Some(vec![None, Some(250.0), Some(250.0)]),
        "only the filtered threshold's limits, which start on the third report, while the bare one already had a limit on the second"
    );

    let quiet = line(&report, r#"alpha {"size":2} Latency value"#);
    assert_eq!(
        quiet.upper_limit,
        Some(150.0),
        "only the bare threshold checks size 2, half again over its baseline"
    );
    assert_eq!(
        quiet.history.upper,
        Some(vec![Some(150.0), Some(150.0), Some(150.0)])
    );
}

// Kills: a row that shows the last iteration when an earlier one alerted, and
// points that collapse the iterations of one report.
#[tokio::test]
async fn console_report_alerted_iteration_is_the_row() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "iterations").await;
    for (time, hash) in [
        (
            "2024-02-01T00:00:00Z",
            "3000000000000000000000000000000000000001",
        ),
        (
            "2024-02-10T00:00:00Z",
            "3000000000000000000000000000000000000002",
        ),
    ] {
        post(&server, &fixture, Post::main(time, hash, STEADY)).await;
    }
    let subject = post(
        &server,
        &fixture,
        Post {
            iterations: vec![results(SUBJECT), results(STEADY)],
            ..Post::main(
                "2024-02-20T00:00:00Z",
                "3000000000000000000000000000000000000003",
                SUBJECT,
            )
        },
    )
    .await;

    let report = console_report(&server, &fixture, &subject, "").await;

    let alerting = line(&report, ALPHA_10_LATENCY);
    assert!((alerting.value - 260.0).abs() < f64::EPSILON);
    assert!(alerting.alert.is_some());
    assert_eq!(report.points.report, vec![0, 1, 2, 2]);
    assert_eq!(
        report
            .points
            .iteration
            .as_ref()
            .map(|iterations| iterations.iter().map(|i| i.0).collect::<Vec<_>>()),
        Some(vec![0, 0, 0, 1])
    );
    assert_eq!(
        alerting.history.y,
        vec![Some(200.0), Some(200.0), Some(260.0), Some(200.0)]
    );
    assert_eq!(report.total, 7, "iterations are points, not lines");
}

// Kills: a report looked up without its project.
#[tokio::test]
async fn console_report_from_another_project_is_not_found() {
    let server = TestServer::new_at(at(NOW)).await;
    let mine = fixture(&server, "mine").await;
    let theirs = fixture(&server, "theirs").await;
    let report = post(
        &server,
        &theirs,
        Post::main(
            "2024-02-01T00:00:00Z",
            "4000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;

    let (status, _) = try_console_report(&server, &mine.slug, &report, "", Some(&mine.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Kills: a private project served without `view`.
#[tokio::test]
async fn console_report_private_project_needs_view() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "private").await;
    let outsider = server
        .signup("Outsider", "consoleoutsider@example.com")
        .await;
    let report = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-01T00:00:00Z",
            "5000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(fixture.uuid)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut conn)
        .expect("Failed to make the project private");
    drop(conn);

    let (status, _) = try_console_report(&server, &fixture.slug, &report, "", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) =
        try_console_report(&server, &fixture.slug, &report, "", Some(&outsider.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, text) =
        try_console_report(&server, &fixture.slug, &report, "", Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
}

// Kills: a history whose report list grows without bound, and a capped window
// that is not echoed.
#[tokio::test]
async fn console_report_history_is_capped() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "cap").await;
    let early = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-01T00:00:00Z",
            "6000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    let subject = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-20T00:00:00Z",
            "6000000000000000000000000000000000000002",
            SUBJECT,
        ),
    )
    .await;

    // Fill the days between with more reports than a history holds, all on the
    // subject's head and testbed.
    let mut conn = server.db_conn();
    let (project_id, testbed_id): (i32, i32) = schema::testbed::table
        .filter(schema::testbed::uuid.eq(subject.testbed.uuid))
        .select((schema::testbed::project_id, schema::testbed::id))
        .first(&mut conn)
        .expect("Failed to find the testbed");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(subject.branch.head.uuid))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("Failed to find the head");
    let version_uuid = VersionUuid::new();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(version_uuid),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1_000),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the version");
    let version_id: i32 = schema::version::table
        .filter(schema::version::uuid.eq(version_uuid))
        .select(schema::version::id)
        .first(&mut conn)
        .expect("Failed to find the version");
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the head version");
    let first_filler = at("2024-02-10T00:00:00Z").timestamp();
    let fillers = (0..MAX_CONSOLE_HISTORY_REPORTS)
        .map(|minute| {
            let start =
                DateTime::try_from(first_filler + 60 * i64::try_from(minute).expect("a minute"))
                    .expect("a time");
            (
                schema::report::uuid.eq(ReportUuid::new()),
                schema::report::project_id.eq(project_id),
                schema::report::head_id.eq(head_id),
                schema::report::version_id.eq(version_id),
                schema::report::testbed_id.eq(testbed_id),
                schema::report::adapter.eq(Adapter::Json),
                schema::report::start_time.eq(start),
                schema::report::end_time.eq(start),
                schema::report::created.eq(start),
            )
        })
        .collect::<Vec<_>>();
    diesel::insert_into(schema::report::table)
        .values(fillers)
        .execute(&mut conn)
        .expect("Failed to insert the reports");
    drop(conn);

    let report = console_report(
        &server,
        &fixture,
        &subject,
        &format!("?start_time={}", millis("2024-01-01T00:00:00Z")),
    )
    .await;

    assert!(report.window.clamped);
    assert_eq!(
        i64::from(report.window.start_time),
        (first_filler + 60) * 1_000,
        "the subject and the newest fillers fill the history, so the oldest filler is out"
    );
    assert_eq!(
        x(&report),
        vec![millis("2024-02-20T00:00:00Z")],
        "the report from before the fillers is out of the history"
    );
    assert_ne!(report.reports[0].uuid, early.uuid);
}

/// One report of `alpha` alone, built from its variants' measures.
fn alpha(variants: &[(u32, serde_json::Value)]) -> String {
    let variants = variants
        .iter()
        .map(|(size, measures)| serde_json::json!({ "parameters": { "size": size }, "measures": measures }))
        .collect::<Vec<_>>();
    serde_json::json!({ "alpha": variants }).to_string()
}

/// Post `alpha` with these thresholds on three days: two of history, then the
/// subject.
async fn alpha_history(
    server: &TestServer,
    fixture: &Fixture,
    thresholds: &serde_json::Value,
    history: &str,
    subject: &str,
) -> JsonReport {
    for (time, hash) in [
        (
            "2024-02-01T00:00:00Z",
            "a000000000000000000000000000000000000001",
        ),
        (
            "2024-02-10T00:00:00Z",
            "a000000000000000000000000000000000000002",
        ),
    ] {
        post(
            server,
            fixture,
            Post {
                iterations: vec![history.to_owned()],
                thresholds: Some(thresholds.clone()),
                ..Post::main(time, hash, STEADY)
            },
        )
        .await;
    }
    post(
        server,
        fixture,
        Post {
            iterations: vec![subject.to_owned()],
            thresholds: Some(thresholds.clone()),
            ..Post::main(
                "2024-02-20T00:00:00Z",
                "a000000000000000000000000000000000000003",
                STEADY,
            )
        },
    )
    .await
}

// Kills: "the oldest threshold" read off UUIDs, which are not in creation order
// for thresholds made before UUIDv7.
#[tokio::test]
async fn console_report_oldest_threshold_by_creation() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "oldest").await;
    let thresholds = serde_json::json!({ "models": [
        {
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.5 },
        },
        {
            "parameters": [{ "size": 10 }],
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.25 },
        },
    ]});
    let steady = alpha(&[(10, serde_json::json!({ "latency": { "value": 200.0 } }))]);
    let subject = alpha_history(&server, &fixture, &thresholds, &steady, &steady).await;
    // A random UUID on the older threshold sorts it after the newer one.
    let mut conn = server.db_conn();
    diesel::update(schema::threshold::table.filter(schema::threshold::parameters.is_null()))
        .set(schema::threshold::uuid.eq("ffffffff-ffff-4fff-bfff-ffffffffffff"))
        .execute(&mut conn)
        .expect("Failed to change the UUID");
    drop(conn);

    let report = console_report(&server, &fixture, &subject, "").await;

    let line = line(&report, ALPHA_10_LATENCY);
    assert!(line.alert.is_none());
    assert_eq!(
        line.upper_limit,
        Some(300.0),
        "the bare threshold came first"
    );
    assert_eq!(
        report.models[line.model.expect("checked") as usize]
            .threshold
            .to_string(),
        "ffffffff-ffff-4fff-bfff-ffffffffffff"
    );
}

// Kills: a two-sided threshold scored by the signed delta, which ranks a fall
// as better however far it went.
#[tokio::test]
async fn console_report_two_sided_threshold_scores_both_ways() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "twosided").await;
    let thresholds = serde_json::json!({ "models": [
        {
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.5 },
        },
        {
            "measure": "throughput",
            "metric": "value",
            "model": { "test": "percentage", "lower_boundary": 0.5, "upper_boundary": 0.5 },
        },
    ]});
    let history = alpha(&[(
        2,
        serde_json::json!({ "latency": { "value": 100.0 }, "throughput": { "value": 50.0 } }),
    )]);
    // Latency rises a fifth; throughput falls three tenths, both inside their limits.
    let subject = alpha(&[(
        2,
        serde_json::json!({ "latency": { "value": 120.0 }, "throughput": { "value": 35.0 } }),
    )]);
    let subject = alpha_history(&server, &fixture, &thresholds, &history, &subject).await;

    let report = console_report(&server, &fixture, &subject, "?sort=delta").await;

    assert_eq!(
        names(&report),
        vec![
            r#"alpha {"size":2} Throughput value"#,
            r#"alpha {"size":2} Latency value"#,
        ]
    );
}

// Kills: a delta divided by a negative baseline, which flips the guarded side.
#[tokio::test]
async fn console_report_delta_against_a_negative_baseline() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "negative").await;
    let thresholds = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.5 },
    }]});
    let history = alpha(&[
        (2, serde_json::json!({ "latency": { "value": -100.0 } })),
        (10, serde_json::json!({ "latency": { "value": 100.0 } })),
    ]);
    // Both rise: a fifth from below zero, a tenth from above it.
    let subject = alpha(&[
        (2, serde_json::json!({ "latency": { "value": -80.0 } })),
        (10, serde_json::json!({ "latency": { "value": 110.0 } })),
    ]);
    let subject = alpha_history(&server, &fixture, &thresholds, &history, &subject).await;

    let report = console_report(&server, &fixture, &subject, "?sort=delta").await;

    assert_eq!(
        names(&report),
        vec![
            r#"alpha {"size":2} Latency value"#,
            r#"alpha {"size":10} Latency value"#,
        ]
    );
}

// Kills: neighbors that skip a report ending at the same moment.
#[tokio::test]
async fn console_report_neighbors_break_ties_by_creation() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "ties").await;
    let first = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-01T00:00:00Z",
            "b000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    // Two runs that end together, the longer one posted first.
    let long = post(
        &server,
        &fixture,
        Post {
            seconds: 7_200,
            ..Post::main(
                "2024-02-10T00:00:00Z",
                "b000000000000000000000000000000000000002",
                STEADY,
            )
        },
    )
    .await;
    let short = post(
        &server,
        &fixture,
        Post {
            seconds: 3_600,
            ..Post::main(
                "2024-02-10T01:00:00Z",
                "b000000000000000000000000000000000000003",
                STEADY,
            )
        },
    )
    .await;
    let last = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-20T00:00:00Z",
            "b000000000000000000000000000000000000004",
            STEADY,
        ),
    )
    .await;

    let long_page = console_report(&server, &fixture, &long, "?per_page=1").await;
    let short_page = console_report(&server, &fixture, &short, "?per_page=1").await;

    assert_eq!(long_page.previous.map(|link| link.uuid), Some(first.uuid));
    assert_eq!(long_page.next.map(|link| link.uuid), Some(short.uuid));
    assert_eq!(short_page.previous.map(|link| link.uuid), Some(long.uuid));
    assert_eq!(short_page.next.map(|link| link.uuid), Some(last.uuid));
}

// Kills: an active count that counts every alert, and dismissed or silenced
// alerts dropped from alerting: the value still crossed its limit.
#[tokio::test]
async fn console_report_dismissed_and_silenced_alerts_still_alert() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "dismissed").await;
    let alerts = seeded.subject.alerts.as_ref().expect("the subject alerted");
    let gamma = alerts
        .iter()
        .find(|alert| alert.benchmark.name.as_ref() == "gamma")
        .expect("gamma alerted");
    let alpha = alerts
        .iter()
        .find(|alert| alert.benchmark.name.as_ref() == "alpha")
        .expect("alpha alerted");
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/alerts/{}",
            seeded.fixture.slug, gamma.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&seeded.fixture.token),
        )
        .json(&serde_json::json!({ "status": "dismissed" }))
        .send()
        .await
        .expect("Request failed");
    assert!(resp.status().is_success(), "PATCH alert: {}", resp.status());
    let mut conn = server.db_conn();
    diesel::update(schema::alert::table.filter(schema::alert::uuid.eq(alpha.uuid)))
        .set(schema::alert::status.eq(AlertStatus::Silenced))
        .execute(&mut conn)
        .expect("Failed to silence the alert");
    drop(conn);

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "").await;

    assert_eq!(report.counts.alerts.total, 2);
    assert_eq!(report.counts.alerts.active, 0);
    assert_eq!(
        names(&report)[..2].to_vec(),
        vec![
            r#"alpha {"size":10} Latency value"#,
            r#"alpha {"size":2} Latency value"#,
        ]
    );
    assert_eq!(
        groups(&report, false),
        vec![
            ("alpha".to_owned(), 4, 2, 1),
            ("gamma".to_owned(), 1, 1, 1),
            ("beta".to_owned(), 2, 1, 0),
        ]
    );
    assert!(matches!(
        line(&report, ALPHA_10_LATENCY)
            .alert
            .map(|alert| alert.status),
        Some(AlertStatus::Silenced)
    ));
    assert!(matches!(
        line(&report, r#"gamma {"size":1} Latency value"#)
            .alert
            .map(|alert| alert.status),
        Some(AlertStatus::Dismissed)
    ));
}

// Kills: a search that is ignored, matches with case, skips the parameters or
// the measure, or runs after the page is cut.
#[tokio::test]
async fn console_report_search_filters_before_paging() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "search").await;
    let get =
        |query: &'static str| console_report(&server, &seeded.fixture, &seeded.subject, query);

    let alpha = get("?search=ALPHA").await;
    assert_eq!(alpha.total, 4);
    assert_eq!(groups(&alpha, false), vec![("alpha".to_owned(), 4, 2, 1)]);
    assert_eq!(
        alpha.counts.benchmarks, 3,
        "the report's counts are the whole report"
    );

    let size = get("?search=size%3D10").await;
    assert_eq!(
        names(&size),
        vec![
            r#"alpha {"size":10} Latency value"#,
            r#"alpha {"size":10} Throughput value"#,
        ]
    );

    let throughput = get("?search=throughput").await;
    assert_eq!(throughput.total, 2);

    let p99 = get("?search=P99").await;
    assert_eq!(
        names(&p99),
        vec![BETA_P99],
        "the metric name is part of the row"
    );

    let second = get("?search=alpha&per_page=1&page=2").await;
    assert_eq!(names(&second), vec![r#"alpha {"size":2} Latency value"#]);
}

// Kills: a zero baseline scored as an infinite delta, which would put the line
// first rather than last with the lines nothing scores.
#[tokio::test]
async fn console_report_zero_baseline_is_unscored() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "zero").await;
    let thresholds = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.5 },
    }]});
    let history = alpha(&[
        (2, serde_json::json!({ "latency": { "value": 0.0 } })),
        (10, serde_json::json!({ "latency": { "value": 100.0 } })),
    ]);
    let subject = alpha(&[
        (2, serde_json::json!({ "latency": { "value": 5.0 } })),
        (10, serde_json::json!({ "latency": { "value": 90.0 } })),
    ]);
    let subject = alpha_history(&server, &fixture, &thresholds, &history, &subject).await;

    let report = console_report(&server, &fixture, &subject, "?sort=delta").await;

    assert_eq!(
        names(&report),
        vec![
            r#"alpha {"size":10} Latency value"#,
            r#"alpha {"size":2} Latency value"#,
        ]
    );
}

// Kills: alert points listed in the order the rows came rather than along x.
#[tokio::test]
async fn console_report_alert_points_run_along_x() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "twoalerts").await;
    let thresholds = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.25 },
    }]});
    let steady = alpha(&[(10, serde_json::json!({ "latency": { "value": 200.0 } }))]);
    let spike = alpha(&[(10, serde_json::json!({ "latency": { "value": 300.0 } }))]);
    // The later spike is posted before the earlier one, so its rows come first.
    for (time, hash, payload) in [
        (
            "2024-02-01T00:00:00Z",
            "d000000000000000000000000000000000000001",
            &steady,
        ),
        (
            "2024-02-05T00:00:00Z",
            "d000000000000000000000000000000000000002",
            &steady,
        ),
        (
            "2024-02-20T00:00:00Z",
            "d000000000000000000000000000000000000003",
            &spike,
        ),
        (
            "2024-02-10T00:00:00Z",
            "d000000000000000000000000000000000000004",
            &spike,
        ),
    ] {
        post(
            &server,
            &fixture,
            Post {
                iterations: vec![payload.clone()],
                thresholds: Some(thresholds.clone()),
                ..Post::main(time, hash, STEADY)
            },
        )
        .await;
    }
    let subject = post(
        &server,
        &fixture,
        Post {
            iterations: vec![steady.clone()],
            thresholds: Some(thresholds.clone()),
            ..Post::main(
                "2024-02-25T00:00:00Z",
                "d000000000000000000000000000000000000005",
                STEADY,
            )
        },
    )
    .await;

    let report = console_report(&server, &fixture, &subject, "").await;

    let history = &line(&report, ALPHA_10_LATENCY).history;
    assert_eq!(
        history.y,
        vec![
            Some(200.0),
            Some(200.0),
            Some(300.0),
            Some(300.0),
            Some(200.0)
        ]
    );
    assert_eq!(
        history
            .alerts
            .iter()
            .map(|alert| alert.index)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}

// Kills: a window in days counted from the report's end rather than its start,
// days that are ignored, and a window given with a start time or out of range
// that is served rather than refused.
#[tokio::test]
async fn console_report_window_in_days() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "days").await;

    let five = console_report(&server, &seeded.fixture, &seeded.subject, "?window=5").await;
    assert_eq!(
        i64::from(five.window.start_time),
        millis("2024-02-20T00:00:00Z")
    );
    assert_eq!(
        x(&five),
        vec![
            millis("2024-02-20T00:00:00Z"),
            millis("2024-02-25T00:00:00Z"),
        ]
    );

    let year = console_report(&server, &seeded.fixture, &seeded.subject, "?window=366").await;
    assert_eq!(year.reports[0].uuid, seeded.first.uuid);

    for query in [
        format!("?window=5&start_time={}", millis("2024-02-05T00:00:00Z")),
        "?window=0".to_owned(),
        "?window=367".to_owned(),
    ] {
        let (status, text) = try_console_report(
            &server,
            &seeded.fixture.slug,
            &seeded.subject,
            &query,
            Some(&seeded.fixture.token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {text}");
    }
}

// Kills: a metric count taken from the page or the search, or one that counts
// measures rather than metric names.
#[tokio::test]
async fn console_report_counts_metric_names() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "metrics").await;
    let payload = serde_json::json!({ "alpha": [{
        "parameters": { "size": 2 },
        "measures": { "latency": { "value": 1.0, "p50": 1.0, "p99": 2.0 } },
    }]});
    let report = post(
        &server,
        &fixture,
        Post {
            iterations: vec![payload.to_string()],
            thresholds: None,
            ..Post::main(
                "2024-02-10T00:00:00Z",
                "2000000000000000000000000000000000000001",
                STEADY,
            )
        },
    )
    .await;

    let console = console_report(&server, &fixture, &report, "?per_page=1&search=p99").await;

    assert_eq!(console.total, 1);
    assert_eq!(console.counts.measures, 1);
    assert_eq!(console.counts.metrics, 3);
}

/// The measure of this slug in a posted report.
fn measure_uuid(report: &JsonReport, slug: &str) -> MeasureUuid {
    report
        .results
        .as_ref()
        .expect("results")
        .iter()
        .flatten()
        .flat_map(|result| result.measures.iter())
        .find(|measure| measure.measure.slug.to_string() == slug)
        .map(|measure| measure.measure.uuid)
        .expect("the report has the measure")
}

// Kills: a measure or metric filter that is ignored or applied after the page is
// cut, filters that do not combine with each other and the search, and an
// unreadable parameters filter served rather than refused.
#[tokio::test]
async fn console_report_filters_before_paging() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "filters").await;
    let throughput = measure_uuid(&seeded.subject, "throughput");

    let second = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        &format!("?measure={throughput}&per_page=1&page=2"),
    )
    .await;
    assert_eq!(second.total, 2);
    assert_eq!(
        names(&second),
        vec![r#"alpha {"size":10} Throughput value"#]
    );

    let first = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        &format!("?measure={throughput}"),
    )
    .await;
    assert_eq!(groups(&first, false), vec![("alpha".to_owned(), 2, 2, 0)]);

    let p99 = console_report(&server, &seeded.fixture, &seeded.subject, "?metric=p99").await;
    assert_eq!(names(&p99), vec![BETA_P99]);

    // {"size":1} and the value metric: beta and gamma, not beta's p99.
    let small = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?parameters=%7B%22size%22%3A1%7D&metric=value",
    )
    .await;
    assert_eq!(
        names(&small),
        vec![
            r#"gamma {"size":1} Latency value"#,
            r#"beta {"size":1} Latency value"#,
        ]
    );
    let gamma = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?parameters=%7B%22size%22%3A1%7D&search=gamma",
    )
    .await;
    assert_eq!(gamma.total, 1);

    let (status, text) = try_console_report(
        &server,
        &seeded.fixture.slug,
        &seeded.subject,
        "?parameters=size",
        Some(&seeded.fixture.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
}

// Kills: a parameters filter that matches the whole parameter set rather than a
// subset of it.
#[tokio::test]
async fn console_report_parameters_filter_is_a_subset() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "subset").await;
    let payload = serde_json::json!({ "alpha": [
        {
            "parameters": { "size": 10, "simd": "avx2" },
            "measures": { "latency": { "value": 1.0 } },
        },
        {
            "parameters": { "size": 2 },
            "measures": { "latency": { "value": 1.0 } },
        },
    ]});
    let report = post(
        &server,
        &fixture,
        Post {
            iterations: vec![payload.to_string()],
            thresholds: None,
            ..Post::main(
                "2024-02-10T00:00:00Z",
                "3000000000000000000000000000000000000001",
                STEADY,
            )
        },
    )
    .await;

    let console = console_report(
        &server,
        &fixture,
        &report,
        "?parameters=%7B%22size%22%3A10%7D",
    )
    .await;

    assert_eq!(
        names(&console),
        vec![r#"alpha {"simd":"avx2","size":10} Latency value"#]
    );
}

// Kills: a count-only page that still carries lines, groups, their tables, or
// history, or that counts the lines before the filters.
#[tokio::test]
async fn console_report_count_only_page() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "countonly").await;

    let count = console_report(
        &server,
        &seeded.fixture,
        &seeded.subject,
        "?per_page=0&metric=value",
    )
    .await;

    assert_eq!(count.uuid, seeded.subject.uuid);
    assert_eq!(count.total, 6);
    assert_eq!(count.counts.benchmarks, 3);
    assert_eq!(count.lines.len(), 0);
    assert_eq!(count.groups.len(), 0);
    assert_eq!(count.benchmarks.len(), 0);
    assert_eq!(count.measures.len(), 0);
    assert_eq!(count.points.x.len(), 0);
}

// Kills: a branch that names its current head rather than the head the report
// ran on.
#[tokio::test]
async fn console_report_branch_names_the_reports_head() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "head").await;
    let mut conn = server.db_conn();
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(seeded.subject.branch.uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to read the branch");
    let moved = bencher_json::HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(moved),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(at(NOW)),
        ))
        .execute(&mut conn)
        .expect("Failed to add a head");
    let moved_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(moved))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("Failed to read the head");
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(moved_id))
        .execute(&mut conn)
        .expect("Failed to move the branch to its new head");
    drop(conn);

    let report = console_report(&server, &seeded.fixture, &seeded.subject, "?per_page=1").await;

    assert_eq!(report.branch.head, seeded.subject.branch.head.uuid);
}

/// Post `alpha` once a day from the first of December under a two sided latency
/// threshold, size 10 with each of these latencies and size 2 on the small days,
/// and return the last.
async fn alpha_days(
    server: &TestServer,
    fixture: &Fixture,
    latencies: &[f64],
    small_days: &[usize],
) -> JsonReport {
    let first = at("2023-12-01T00:00:00Z").timestamp();
    let mut last = None;
    for (day, latency) in latencies.iter().enumerate() {
        let start = DateTime::try_from(first + 86_400 * i64::try_from(day).expect("a day"))
            .expect("a time");
        let mut variants = vec![(10, serde_json::json!({ "latency": { "value": latency } }))];
        if small_days.contains(&day) {
            variants.push((2, serde_json::json!({ "latency": { "value": 100.0 } })));
        }
        let hash = format!(
            "{:040x}",
            0xc000_0000u64 + u64::try_from(day).expect("a day")
        );
        let start = serde_json::to_value(start).expect("a time");
        let body = serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "hash": hash,
            "start_time": start,
            "end_time": start,
            "results": [alpha(&variants)],
            "bmf_version": 1,
            // The lower limit is wide enough that no point falls below it.
            "thresholds": { "models": [{
                "measure": "latency",
                "metric": "value",
                "model": { "test": "percentage", "lower_boundary": 0.9, "upper_boundary": 0.25 },
            }] },
        });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{}/reports", fixture.slug)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&fixture.token),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        let status = resp.status();
        let text = resp.text().await.expect("Failed to read the response");
        assert_eq!(
            status,
            StatusCode::CREATED,
            "POST report on day {day}: {text}"
        );
        last = Some(serde_json::from_str(&text).expect("Failed to parse the report"));
    }
    last.expect("a report")
}

const SINCE_NOVEMBER: &str = "?start_time=1698796800000";

// Kills: thinning that drops a bucket's extreme, an alerting point, or the
// report's own point, a baseline or limit that does not travel with the kept
// points, an index column that does not point at the kept points, and thinning a
// line that fits.
#[tokio::test]
async fn console_report_thinning_keeps_extremes_alerts_and_the_report() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "thin").await;
    // Two buckets at four points: the first keeps 150 and 210, the second 300 and
    // 140; 300 and 290 alert; 205 is the report's own point.
    let subject = alpha_days(
        &server,
        &fixture,
        &[200.0, 150.0, 210.0, 300.0, 290.0, 140.0, 205.0],
        &[0, 6],
    )
    .await;

    let full = console_report(
        &server,
        &fixture,
        &subject,
        &format!("{SINCE_NOVEMBER}&points=256"),
    )
    .await;
    let thin = console_report(
        &server,
        &fixture,
        &subject,
        &format!("{SINCE_NOVEMBER}&points=4"),
    )
    .await;

    let full_line = line(&full, ALPHA_10_LATENCY);
    assert!(full_line.history.index.is_none(), "seven points fit in 256");
    assert_eq!(full_line.history.y.len(), 7);

    let thin_line = line(&thin, ALPHA_10_LATENCY);
    let index = thin_line.history.index.clone().expect("thinned");
    assert_eq!(
        index,
        vec![1, 2, 3, 4, 5, 6],
        "the first day is neither extreme nor alerting"
    );
    assert_eq!(
        thin_line.history.y,
        vec![
            Some(150.0),
            Some(210.0),
            Some(300.0),
            Some(290.0),
            Some(140.0),
            Some(205.0)
        ]
    );
    let kept = |column: &Option<Vec<Option<f64>>>| {
        let column = column.as_ref().expect("checked");
        Some(
            index
                .iter()
                .map(|point| column[*point as usize])
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        thin_line.history.baseline,
        kept(&full_line.history.baseline),
        "each kept point keeps its own baseline"
    );
    assert_eq!(
        thin_line.history.lower,
        kept(&full_line.history.lower),
        "each kept point keeps its own lower limit"
    );
    assert_eq!(
        thin_line.history.upper,
        kept(&full_line.history.upper),
        "each kept point keeps its own upper limit"
    );
    assert_eq!(
        thin_line
            .history
            .alerts
            .iter()
            .map(|alert| alert.index)
            .collect::<Vec<_>>(),
        vec![2, 3],
        "alerts point into the thinned columns"
    );

    let small = line(&thin, r#"alpha {"size":2} Latency value"#);
    assert!(small.history.index.is_none(), "two points fit in four");
    assert_eq!(small.history.y.len(), thin.points.x.len());
    assert_eq!(
        (small.history.y[0], small.history.y[6]),
        (Some(100.0), Some(100.0))
    );

    // Alone, the thinned line leaves the first day out of the shared tables.
    let alone = console_report(
        &server,
        &fixture,
        &subject,
        &format!("{SINCE_NOVEMBER}&points=4&search=size%3D10"),
    )
    .await;
    assert_eq!(alone.points.x.len(), 6);
    assert_eq!(alone.reports.len(), 6);
    assert_eq!(alone.lines[0].history.index, Some(vec![0, 1, 2, 3, 4, 5]));
    assert_eq!(x(&alone)[0], x(&full)[1]);
}

// Kills: an unthinned line left on the points of the unshrunk tables, and
// thinning a line with exactly as many values as the limit.
#[tokio::test]
async fn console_report_thinning_realigns_the_lines_that_fit() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "thinfit").await;
    // The thinned line keeps every day but the first, which only it reports.
    let subject = alpha_days(
        &server,
        &fixture,
        &[200.0, 150.0, 210.0, 300.0, 290.0, 140.0, 205.0],
        &[1, 2, 5, 6],
    )
    .await;

    let thin = console_report(
        &server,
        &fixture,
        &subject,
        &format!("{SINCE_NOVEMBER}&points=4"),
    )
    .await;

    assert_eq!(x(&thin)[0], millis("2023-12-02T00:00:00Z"));
    assert_eq!(thin.reports.len(), 6);
    assert_eq!(
        line(&thin, ALPHA_10_LATENCY).history.index,
        Some(vec![0, 1, 2, 3, 4, 5])
    );
    let small = line(&thin, r#"alpha {"size":2} Latency value"#);
    assert!(small.history.index.is_none(), "four points fit in four");
    assert_eq!(
        small.history.y,
        vec![
            Some(100.0),
            Some(100.0),
            None,
            None,
            Some(100.0),
            Some(100.0)
        ]
    );
}

// Kills: a default other than 64 points, a limit of 256 that is ignored, and a
// limit outside 2 to 256 that is served rather than refused.
#[tokio::test]
async fn console_report_thinning_defaults_to_64_points() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "thindefault").await;
    let latencies = (0..70)
        .map(|day: u8| 200.0 + f64::from(day.rem_euclid(7)))
        .collect::<Vec<_>>();
    let subject = alpha_days(&server, &fixture, &latencies, &[]).await;

    let default = console_report(&server, &fixture, &subject, SINCE_NOVEMBER).await;
    let history = &line(&default, ALPHA_10_LATENCY).history;
    let kept = history
        .index
        .as_ref()
        .expect("70 points do not fit in 64")
        .len();
    assert!(kept <= 65, "64 and the report's own point, kept {kept}");
    assert!(kept > 32, "two points from each of 32 buckets, kept {kept}");

    let most = console_report(
        &server,
        &fixture,
        &subject,
        &format!("{SINCE_NOVEMBER}&points=256"),
    )
    .await;
    let history = &line(&most, ALPHA_10_LATENCY).history;
    assert!(history.index.is_none(), "70 points fit in 256");
    assert_eq!(history.y.len(), 70);

    for (points, expected) in [
        (1, StatusCode::BAD_REQUEST),
        (2, StatusCode::OK),
        (257, StatusCode::BAD_REQUEST),
    ] {
        let (status, text) = try_console_report(
            &server,
            &fixture.slug,
            &subject,
            &format!("{SINCE_NOVEMBER}&points={points}"),
            Some(&fixture.token),
        )
        .await;
        assert_eq!(status, expected, "points={points}: {text}");
    }
}
