#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The console's alerts: `GET /v0/projects/{project}/console/alerts`.
//!
//! Every report here is posted through the API with a fixed time, so the
//! thresholds compute real boundaries and alerts from a known history, and each
//! alert's row is checked against the line its report page draws.

use bencher_api_tests::TestServer;
use bencher_json::{
    AlertUuid, DateTime, JsonReport, ProjectSlug, ProjectUuid, ReportUuid, VersionUuid,
    project::{
        Visibility,
        alert::JsonUpdatedAlerts,
        console::{
            JsonConsoleAlertGroup, JsonConsoleAlerts, JsonConsoleBenchmark, JsonConsoleMeasure,
            JsonConsoleModel, JsonConsolePointReport, JsonConsolePoints, JsonConsoleReport,
            JsonConsoleReportLine, JsonConsoleVariant, MAX_CONSOLE_HISTORY_REPORTS,
        },
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

struct Fixture {
    slug: ProjectSlug,
    uuid: ProjectUuid,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("consolealerts{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Console Alerts Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Console Alerts Project {label}"))
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
    gamma_latency: f64,
}

const STEADY: Values = Values {
    alpha_2_latency: 100.0,
    alpha_10_latency: 200.0,
    alpha_2_throughput: 50.0,
    alpha_10_throughput: 50.0,
    beta_latency: 10.0,
    gamma_latency: 5.0,
};

/// Against a steady history, `alpha` at size 10 and `gamma` cross their upper limits.
const SUBJECT: Values = Values {
    alpha_10_latency: 260.0,
    gamma_latency: 7.0,
    ..STEADY
};

/// Every line alerts but `alpha`'s throughput at size 10.
const BIG: Values = Values {
    alpha_2_latency: 200.0,
    alpha_10_latency: 400.0,
    alpha_2_throughput: 30.0,
    alpha_10_throughput: 50.0,
    beta_latency: 20.0,
    gamma_latency: 9.0,
};

/// Size 10 comes before size 2, so the run raises its alerts in another order than
/// a row's name sorts them.
fn results(values: Values) -> String {
    let Values {
        alpha_2_latency,
        alpha_10_latency,
        alpha_2_throughput,
        alpha_10_throughput,
        beta_latency,
        gamma_latency,
    } = values;
    serde_json::json!({
        "gamma": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": gamma_latency } },
        }],
        "alpha": [
            {
                "parameters": { "size": 10 },
                "measures": {
                    "throughput": { "value": alpha_10_throughput },
                    "latency": { "value": alpha_10_latency },
                },
            },
            {
                "parameters": { "size": 2 },
                "measures": {
                    "throughput": { "value": alpha_2_throughput },
                    "latency": { "value": alpha_2_latency },
                },
            },
        ],
        "beta": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": beta_latency } },
        }],
    })
    .to_string()
}

/// Latency guards its upper side and throughput its lower side, each a quarter
/// off the mean of the history.
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
    testbed: &'static str,
    hash: &'static str,
    iterations: Vec<String>,
    thresholds: serde_json::Value,
}

impl Post {
    fn main(time: &'static str, hash: &'static str, values: Values) -> Self {
        Self {
            time,
            testbed: "localhost",
            hash,
            iterations: vec![results(values)],
            thresholds: thresholds(),
        }
    }
}

/// Posts a report and records it as created when it started, so reports created
/// later are newer.
async fn post(server: &TestServer, fixture: &Fixture, post: Post) -> JsonReport {
    let Post {
        time,
        testbed,
        hash,
        iterations,
        thresholds,
    } = post;
    let start_time = at(time);
    let end_time = DateTime::try_from(start_time.timestamp() + 60).expect("Failed to end the run");
    let body = serde_json::json!({
        "branch": "main",
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
    let report: JsonReport = serde_json::from_str(&text).expect("Failed to parse the report");
    let mut conn = server.db_conn();
    diesel::update(schema::report::table.filter(schema::report::uuid.eq(report.uuid)))
        .set(schema::report::created.eq(start_time))
        .execute(&mut conn)
        .expect("Failed to set when the report was created");
    report
}

/// The reports that raised alerts, by when they were created.
struct Seeded {
    fixture: Fixture,
    /// Two alerts on `main` and `localhost`.
    early: JsonReport,
    /// Two alerts on `main` and `arm`.
    arm: JsonReport,
    /// Five alerts on `main` and `localhost`.
    late: JsonReport,
}

async fn seeded(server: &TestServer, label: &str) -> Seeded {
    let fixture = fixture(server, label).await;
    for (time, hash) in [
        (
            "2024-01-20T00:00:00Z",
            "1000000000000000000000000000000000000001",
        ),
        (
            "2024-02-01T00:00:00Z",
            "1000000000000000000000000000000000000002",
        ),
        (
            "2024-02-10T00:00:00Z",
            "1000000000000000000000000000000000000003",
        ),
    ] {
        post(server, &fixture, Post::main(time, hash, STEADY)).await;
    }
    for (time, hash) in [
        (
            "2024-02-01T00:00:00Z",
            "2000000000000000000000000000000000000001",
        ),
        (
            "2024-02-10T00:00:00Z",
            "2000000000000000000000000000000000000002",
        ),
    ] {
        post(
            server,
            &fixture,
            Post {
                testbed: "arm",
                ..Post::main(time, hash, STEADY)
            },
        )
        .await;
    }
    let early = post(
        server,
        &fixture,
        Post::main(
            "2024-02-15T00:00:00Z",
            "1000000000000000000000000000000000000004",
            SUBJECT,
        ),
    )
    .await;
    let arm = post(
        server,
        &fixture,
        Post {
            testbed: "arm",
            ..Post::main(
                "2024-02-20T00:00:00Z",
                "2000000000000000000000000000000000000003",
                SUBJECT,
            )
        },
    )
    .await;
    let late = post(
        server,
        &fixture,
        Post::main(
            "2024-02-25T00:00:00Z",
            "1000000000000000000000000000000000000005",
            BIG,
        ),
    )
    .await;
    Seeded {
        fixture,
        early,
        arm,
        late,
    }
}

async fn try_console_alerts(
    server: &TestServer,
    project: &ProjectSlug,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, String) {
    let mut request = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project}/console/alerts{query}")));
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

async fn console_alerts(server: &TestServer, fixture: &Fixture, query: &str) -> JsonConsoleAlerts {
    let (status, text) =
        try_console_alerts(server, &fixture.slug, query, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "GET console alerts{query}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console alerts")
}

async fn console_report(
    server: &TestServer,
    fixture: &Fixture,
    report: ReportUuid,
    query: &str,
) -> JsonConsoleReport {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/console/reports/{report}{query}",
            fixture.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read the response");
    assert_eq!(status, StatusCode::OK, "GET console report: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console report")
}

/// Every alert on the page, in list order.
fn listed(alerts: &JsonConsoleAlerts) -> Vec<AlertUuid> {
    alerts
        .groups
        .iter()
        .flat_map(|group| &group.alerts)
        .map(|alert| alert.line.alert.expect("an alert's line alerted").uuid)
        .collect()
}

/// A row as it names its line: benchmark, variant parameters, measure, metric.
fn alert_names(alerts: &JsonConsoleAlerts, group: &JsonConsoleAlertGroup) -> Vec<String> {
    group
        .alerts
        .iter()
        .map(|alert| {
            let line = &alert.line;
            format!(
                "{} {} {} {}",
                alerts.benchmarks[line.benchmark as usize].name,
                alerts.variants[line.variant as usize].parameters,
                alerts.measures[line.measure as usize].name,
                line.metric,
            )
        })
        .collect()
}

/// What a response's tables hold for a line.
struct Tables<'a> {
    points: &'a JsonConsolePoints,
    reports: &'a [JsonConsolePointReport],
    benchmarks: &'a [JsonConsoleBenchmark],
    variants: &'a [JsonConsoleVariant],
    measures: &'a [JsonConsoleMeasure],
    models: &'a [JsonConsoleModel],
}

impl<'a> Tables<'a> {
    fn of_report(report: &'a JsonConsoleReport) -> Self {
        Self {
            points: &report.points,
            reports: &report.reports,
            benchmarks: &report.benchmarks,
            variants: &report.variants,
            measures: &report.measures,
            models: &report.models,
        }
    }

    fn of_group(alerts: &'a JsonConsoleAlerts, group: &'a JsonConsoleAlertGroup) -> Self {
        Self {
            points: &group.points,
            reports: &alerts.reports,
            benchmarks: &alerts.benchmarks,
            variants: &alerts.variants,
            measures: &alerts.measures,
            models: &alerts.models,
        }
    }

    /// A line with every index read through the tables, and its history as the
    /// points it has a value at, since two responses share their x with other lines.
    fn drawn(&self, line: &JsonConsoleReportLine) -> serde_json::Value {
        let history = &line.history;
        let point_of = |position: usize| -> usize {
            history
                .index
                .as_ref()
                .map_or(position, |index| index[position] as usize)
        };
        let point = |position: usize| {
            let point = point_of(position);
            serde_json::json!({
                "x": self.points.x[point],
                "report": self.reports[self.points.report[point] as usize].uuid,
                "iteration": self.points.iteration.as_ref().map_or(0, |iteration| iteration[point].0),
                "y": history.y[position],
                "baseline": history.baseline.as_ref().map(|column| column[position]),
                "lower": history.lower.as_ref().map(|column| column[position]),
                "upper": history.upper.as_ref().map(|column| column[position]),
            })
        };
        let variant = &self.variants[line.variant as usize];
        serde_json::json!({
            "benchmark": self.benchmarks[line.benchmark as usize].uuid,
            "variant": variant.uuid,
            "variant_benchmark": self.benchmarks[variant.benchmark as usize].uuid,
            "measure": self.measures[line.measure as usize].uuid,
            "metric": line.metric,
            "value": line.value,
            "model": line.model.map(|model| self.models[model as usize]),
            "baseline": line.baseline,
            "lower_limit": line.lower_limit,
            "upper_limit": line.upper_limit,
            "alert": line.alert,
            "history": (0..history.y.len())
                .filter(|position| history.y[*position].is_some())
                .map(point)
                .collect::<Vec<_>>(),
            "alerts": history
                .alerts
                .iter()
                .map(|alert| serde_json::json!({
                    "at": point(alert.index as usize),
                    "uuid": alert.uuid,
                    "limit": alert.limit,
                    "status": alert.status,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

/// Each alert's row against the line its report page draws, with the same history
/// window and size.
async fn assert_rows_are_report_lines(
    server: &TestServer,
    fixture: &Fixture,
    alerts: &JsonConsoleAlerts,
    query: &str,
) {
    assert!(!alerts.groups.is_empty(), "alerts to check");
    for group in &alerts.groups {
        let report = console_report(server, fixture, group.uuid, query).await;
        assert_eq!(
            (
                group.window.start_time,
                group.window.end_time,
                group.window.clamped
            ),
            (
                report.window.start_time,
                report.window.end_time,
                report.window.clamped
            ),
            "{query}"
        );
        for alert in &group.alerts {
            let uuid = alert.line.alert.expect("an alert's line alerted").uuid;
            let line = report
                .lines
                .iter()
                .find(|line| line.alert.is_some_and(|line_alert| line_alert.uuid == uuid))
                .expect("the report draws the alert's line");
            assert_eq!(
                Tables::of_group(alerts, group).drawn(&alert.line),
                Tables::of_report(&report).drawn(line),
                "{query}"
            );
        }
    }
}

// Kills: groups ordered oldest report first or by start time over creation, a
// report's alerts in the order they were raised instead of the order its rows draw,
// and a group whose identity, count, or creation time is not its report's.
#[tokio::test]
async fn console_alerts_group_by_report_newest_first() {
    let server = TestServer::new_at(at(NOW)).await;
    let Seeded {
        fixture,
        early,
        arm,
        late,
    } = seeded(&server, "groups").await;
    // The early report was created last, after the other two.
    let mut conn = server.db_conn();
    diesel::update(schema::report::table.filter(schema::report::uuid.eq(early.uuid)))
        .set(schema::report::created.eq(at("2024-02-28T00:00:00Z")))
        .execute(&mut conn)
        .expect("Failed to set when the report was created");
    drop(conn);

    let alerts = console_alerts(&server, &fixture, "").await;

    assert_eq!(
        alerts
            .groups
            .iter()
            .map(|group| (group.uuid, group.total))
            .collect::<Vec<_>>(),
        vec![(early.uuid, 2), (late.uuid, 5), (arm.uuid, 2)]
    );
    assert_eq!(alerts.total, 9);
    assert_eq!(alerts.counts.active, 9);
    assert_eq!(
        alerts
            .groups
            .iter()
            .map(|group| i64::from(group.created))
            .collect::<Vec<_>>(),
        [
            "2024-02-28T00:00:00Z",
            "2024-02-25T00:00:00Z",
            "2024-02-20T00:00:00Z"
        ]
        .map(|time| at(time).timestamp_millis()),
        "when the API took each report"
    );
    for group in &alerts.groups {
        let report = console_report(&server, &fixture, group.uuid, "?per_page=255").await;
        let branch = &alerts.branches[group.branch as usize];
        assert_eq!(
            (branch.uuid, &branch.name, branch.head),
            (report.branch.uuid, &report.branch.name, report.branch.head)
        );
        let testbed = &alerts.testbeds[group.testbed as usize];
        assert_eq!(
            (testbed.uuid, &testbed.name),
            (report.testbed.uuid, &report.testbed.name)
        );
        assert_eq!(group.version.hash, report.version.hash);
        assert_eq!(
            (group.start_time, group.end_time),
            (report.start_time, report.end_time)
        );
        assert_eq!(
            serde_json::json!(group.adapter),
            serde_json::json!(report.adapter)
        );
        let drawn = report
            .lines
            .iter()
            .filter_map(|line| line.alert.map(|alert| alert.uuid))
            .collect::<Vec<_>>();
        let rows = group
            .alerts
            .iter()
            .filter_map(|alert| alert.line.alert.map(|alert| alert.uuid))
            .collect::<Vec<_>>();
        assert_eq!(rows, drawn, "a report's alerts in its drawing order");
    }
    assert_eq!(
        alert_names(&alerts, &alerts.groups[1]),
        vec![
            r#"alpha {"size":2} Latency value"#,
            r#"alpha {"size":2} Throughput value"#,
            r#"alpha {"size":10} Latency value"#,
            r#"beta {"size":1} Latency value"#,
            r#"gamma {"size":1} Latency value"#,
        ]
    );
    assert_eq!(
        alerts.testbeds.len(),
        2,
        "the two localhost groups share a testbed"
    );
}

// Kills: a row whose value, limits, model, or history differs from its report
// page's line, a history window that does not end at its own report or ignores
// `window`, and histories not thinned to `points`.
#[tokio::test]
async fn console_alerts_rows_are_the_report_lines() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "rows").await;
    for query in ["", "?window=60&points=4", "?window=7"] {
        let alerts = console_alerts(&server, &seeded.fixture, query).await;
        assert_rows_are_report_lines(&server, &seeded.fixture, &alerts, query).await;
    }
    let thinned = console_alerts(&server, &seeded.fixture, "?window=60&points=4").await;
    assert!(
        thinned
            .groups
            .iter()
            .flat_map(|group| &group.alerts)
            .any(|alert| alert.line.history.index.is_some()),
        "a history longer than `points` is thinned"
    );
}

// Kills: a page that cuts a report's alerts by when they were raised instead of by
// how they are drawn, a group total counted on the page, and a page that repeats or
// skips an alert at its edges.
#[tokio::test]
async fn console_alerts_pages_cut_inside_a_group() {
    let server = TestServer::new_at(at(NOW)).await;
    let Seeded {
        fixture,
        late,
        arm,
        early,
    } = seeded(&server, "pages").await;
    let all = console_alerts(&server, &fixture, "").await;
    let late_rows = listed(&all).into_iter().take(5).collect::<Vec<_>>();
    // The run raises its alerts in whatever order it reads its results, so the late
    // report's alerts take their identifiers in the reverse of the order its rows draw.
    let mut conn = server.db_conn();
    let ids = schema::alert::table
        .filter(schema::alert::uuid.eq_any(&late_rows))
        .order(schema::alert::id.asc())
        .select(schema::alert::id)
        .load::<i32>(&mut conn)
        .expect("Failed to read the alerts");
    let after_every_alert = schema::alert::table
        .select(diesel::dsl::max(schema::alert::id))
        .first::<Option<i32>>(&mut conn)
        .expect("Failed to read the newest alert")
        .unwrap_or_default();
    for (offset, row) in (1..).zip(&late_rows) {
        diesel::update(schema::alert::table.filter(schema::alert::uuid.eq(row)))
            .set(schema::alert::id.eq(after_every_alert + offset))
            .execute(&mut conn)
            .expect("Failed to move an alert");
    }
    for (id, row) in ids.iter().zip(late_rows.iter().rev()) {
        diesel::update(schema::alert::table.filter(schema::alert::uuid.eq(row)))
            .set(schema::alert::id.eq(id))
            .execute(&mut conn)
            .expect("Failed to renumber an alert");
    }
    drop(conn);

    let mut paged = Vec::new();
    for page in 1..=4 {
        let alerts = console_alerts(&server, &fixture, &format!("?per_page=3&page={page}")).await;
        assert_eq!(alerts.total, 9);
        for group in &alerts.groups {
            let whole = all
                .groups
                .iter()
                .find(|whole| whole.uuid == group.uuid)
                .expect("the group is on the whole list");
            assert_eq!(
                group.total, whole.total,
                "a group's total covers every page"
            );
        }
        if page == 2 {
            assert_eq!(
                alerts
                    .groups
                    .iter()
                    .map(|group| (group.uuid, group.alerts.len()))
                    .collect::<Vec<_>>(),
                vec![(late.uuid, 2), (arm.uuid, 1)]
            );
        }
        paged.extend(listed(&alerts));
    }
    assert_eq!(paged, listed(&all));
    assert_eq!(
        all.groups
            .iter()
            .map(|group| group.uuid)
            .collect::<Vec<_>>(),
        vec![late.uuid, arm.uuid, early.uuid]
    );
}

// Kills: one row for a line that two thresholds both alerted on, or that alerted
// in two iterations, and a row's limits taken from the other threshold.
#[tokio::test]
async fn console_alerts_one_line_raised_twice() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "twice").await;
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
    let both = serde_json::json!({ "models": [bare, filtered] });
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
        post(
            &server,
            &fixture,
            Post {
                thresholds: both.clone(),
                ..Post::main(time, hash, STEADY)
            },
        )
        .await;
    }
    let two_thresholds = post(
        &server,
        &fixture,
        Post {
            thresholds: both.clone(),
            ..Post::main(
                "2024-02-20T00:00:00Z",
                "3000000000000000000000000000000000000003",
                Values {
                    alpha_10_latency: 400.0,
                    ..STEADY
                },
            )
        },
    )
    .await;
    let fixed = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "static", "upper_boundary": 250.0 },
    }] });
    post(
        &server,
        &fixture,
        Post {
            testbed: "arm",
            thresholds: fixed.clone(),
            ..Post::main(
                "2024-02-11T00:00:00Z",
                "3000000000000000000000000000000000000005",
                STEADY,
            )
        },
    )
    .await;
    let two_iterations = post(
        &server,
        &fixture,
        Post {
            testbed: "arm",
            iterations: vec![results(SUBJECT), results(SUBJECT)],
            thresholds: fixed,
            ..Post::main(
                "2024-02-21T00:00:00Z",
                "3000000000000000000000000000000000000004",
                SUBJECT,
            )
        },
    )
    .await;

    let alerts = console_alerts(&server, &fixture, "").await;
    let group = |uuid: ReportUuid| {
        alerts
            .groups
            .iter()
            .find(|group| group.uuid == uuid)
            .expect("the report raised alerts")
    };

    let thresholds = group(two_thresholds.uuid);
    let mut limits = thresholds
        .alerts
        .iter()
        .map(|alert| {
            let upper = alert
                .line
                .history
                .upper
                .as_ref()
                .expect("a history with limits");
            (
                alert.line.upper_limit,
                upper.last().copied().flatten(),
                upper.iter().flatten().count(),
            )
        })
        .collect::<Vec<_>>();
    limits.sort_by(|a, b| a.0.unwrap_or_default().total_cmp(&b.0.unwrap_or_default()));
    assert_eq!(
        limits,
        vec![(Some(250.0), Some(250.0), 2), (Some(300.0), Some(300.0), 2),],
        "one row per threshold, each with its own limits"
    );

    let iterations = group(two_iterations.uuid);
    assert_eq!(iterations.alerts.len(), 2, "one row per iteration");
    let histories = iterations
        .alerts
        .iter()
        .map(|alert| {
            (
                alert.line.history.y.iter().flatten().count(),
                alert.line.history.alerts.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        histories,
        vec![(3, 2), (3, 2)],
        "both rows draw both iterations and both alerts"
    );
}

// Kills: windows read only up to the cap of the first report on a head and
// testbed, and a capped window that is not echoed.
#[tokio::test]
async fn console_alerts_windows_match_their_reports() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "windows").await;
    post(
        &server,
        &fixture,
        Post::main(
            "2024-02-01T00:00:00Z",
            "4000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    let first = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-20T00:00:00Z",
            "4000000000000000000000000000000000000002",
            SUBJECT,
        ),
    )
    .await;
    post(
        &server,
        &fixture,
        Post::main(
            "2024-02-24T00:00:00Z",
            "4000000000000000000000000000000000000003",
            BIG,
        ),
    )
    .await;

    // More reports than a history holds, on the reports' head and testbed between
    // the first alerting report and the second, and a few before the first.
    let between = at("2024-02-22T00:00:00Z").timestamp();
    let before = at("2024-02-15T00:00:00Z").timestamp();
    insert_fillers(
        &server,
        &first,
        (0..MAX_CONSOLE_HISTORY_REPORTS)
            .map(|minute| between + 60 * i64::try_from(minute).expect("a minute"))
            .chain((0..8).map(|hour| before + 3_600 * hour)),
    );

    let query = "?window=30";
    let alerts = console_alerts(&server, &fixture, query).await;
    assert_eq!(alerts.groups.len(), 2);
    assert!(
        alerts.groups[0].window.clamped && !alerts.groups[1].window.clamped,
        "the second report's window holds the fillers, the first's only those before it"
    );
    assert_rows_are_report_lines(&server, &fixture, &alerts, query).await;
}

// Kills: a window that reads one report fewer than a history holds before it
// counts the window as clamped, and one that breaks a tie at the cap toward the
// older report.
#[tokio::test]
async fn console_alerts_windows_stop_at_the_cap() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "cap").await;
    // Three runs that end together, at the edge of the cap.
    for hash in [
        "6000000000000000000000000000000000000001",
        "6000000000000000000000000000000000000002",
        "6000000000000000000000000000000000000003",
    ] {
        post(
            &server,
            &fixture,
            Post::main("2024-02-05T00:00:00Z", hash, STEADY),
        )
        .await;
    }
    let alerting = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-20T00:00:00Z",
            "6000000000000000000000000000000000000004",
            SUBJECT,
        ),
    )
    .await;
    // With the three runs and the alerting report itself, two more reports than a
    // history holds.
    let start = at("2024-02-10T00:00:00Z").timestamp();
    let count = i64::try_from(MAX_CONSOLE_HISTORY_REPORTS - 2).expect("a count");
    insert_fillers(
        &server,
        &alerting,
        (0..count).map(|half_hour| start + 1_800 * half_hour),
    );

    let query = "?window=30";
    let alerts = console_alerts(&server, &fixture, query).await;
    assert!(alerts.groups[0].window.clamped, "two reports over the cap");
    assert_rows_are_report_lines(&server, &fixture, &alerts, query).await;

    let mut conn = server.db_conn();
    diesel::delete(
        schema::report::table.filter(schema::report::start_time.between(
            DateTime::try_from(start).expect("a time"),
            DateTime::try_from(start + 1_800).expect("a time"),
        )),
    )
    .execute(&mut conn)
    .expect("Failed to delete two fillers");
    drop(conn);
    let alerts = console_alerts(&server, &fixture, query).await;
    assert!(!alerts.groups[0].window.clamped, "exactly the cap");
    assert_rows_are_report_lines(&server, &fixture, &alerts, query).await;
}

// Kills: Dismiss group by a report filter that changes more or fewer alerts than
// the group's total, and a list that ignores `reports`.
#[tokio::test]
async fn console_alerts_dismiss_a_group_split_across_pages() {
    let server = TestServer::new_at(at(NOW)).await;
    let Seeded {
        fixture, late, arm, ..
    } = seeded(&server, "dismissgroup").await;
    let page = console_alerts(&server, &fixture, "?per_page=3").await;
    let group = &page.groups[0];
    assert_eq!(
        (group.uuid, group.alerts.len(), group.total),
        (late.uuid, 3, 5),
        "the group goes on past the page"
    );

    let listed = console_alerts(
        &server,
        &fixture,
        &format!("?reports={},{}", late.uuid, arm.uuid),
    )
    .await;
    assert_eq!(
        listed
            .groups
            .iter()
            .map(|group| (group.uuid, group.alerts.len()))
            .collect::<Vec<_>>(),
        vec![(late.uuid, 5), (arm.uuid, 2)]
    );

    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}/console/alerts", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({
            "status": "dismissed",
            "filter": { "status": "active", "reports": [group.uuid] },
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonUpdatedAlerts = resp.json().await.expect("Failed to parse the response");
    assert_eq!(updated.changed, group.total);
    let after = console_alerts(&server, &fixture, "").await;
    assert_eq!(after.total, 4);
    assert!(after.groups.iter().all(|group| group.uuid != late.uuid));
}

/// Reports with no results on the report's head and testbed, each starting,
/// ending, and created at one of `starts`, on a version of that head alone.
fn insert_fillers(server: &TestServer, report: &JsonReport, starts: impl Iterator<Item = i64>) {
    let mut conn = server.db_conn();
    let (project_id, testbed_id): (i32, i32) = schema::testbed::table
        .filter(schema::testbed::uuid.eq(report.testbed.uuid))
        .select((schema::testbed::project_id, schema::testbed::id))
        .first(&mut conn)
        .expect("Failed to find the testbed");
    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(report.branch.head.uuid))
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
    let fillers = starts
        .map(|start| {
            let start = DateTime::try_from(start).expect("a time");
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
}

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

// Kills: alerts served without a login, a signed-in reader turned away from a
// public project, a private project's alerts served to a reader without `view`,
// and a project key read on another project.
#[tokio::test]
async fn console_alerts_need_a_signed_in_reader() {
    let server = TestServer::new_at(at(NOW)).await;
    let mine = fixture(&server, "readermine").await;
    let theirs = fixture(&server, "readertheirs").await;
    let outsider = server
        .signup("Outsider", "consolealertsreader@example.com")
        .await;
    post(
        &server,
        &mine,
        Post::main(
            "2024-02-01T00:00:00Z",
            "5000000000000000000000000000000000000001",
            STEADY,
        ),
    )
    .await;
    post(
        &server,
        &mine,
        Post::main(
            "2024-02-10T00:00:00Z",
            "5000000000000000000000000000000000000002",
            SUBJECT,
        ),
    )
    .await;

    let (status, _) = try_console_alerts(&server, &mine.slug, "", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a public project still needs a login"
    );
    let (status, text) = try_console_alerts(&server, &mine.slug, "", Some(&outsider.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let read: JsonConsoleAlerts = serde_json::from_str(&text).expect("Failed to parse the alerts");
    assert_eq!(read.total, 2);

    set_visibility(&server, &mine, Visibility::Private);
    let (status, _) = try_console_alerts(&server, &mine.slug, "", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = try_console_alerts(&server, &mine.slug, "", Some(&outsider.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, text) = try_console_alerts(&server, &mine.slug, "", Some(&mine.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let mine_key = project_key(&server, &mine).await;
    let theirs_key = project_key(&server, &theirs).await;
    let (status, text) = try_console_alerts(&server, &mine.slug, "", Some(&mine_key)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let (status, _) = try_console_alerts(&server, &mine.slug, "", Some(&theirs_key)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
