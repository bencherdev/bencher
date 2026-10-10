#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The console's plot query: `/v0/projects/{project}/console/perf`.
//!
//! Every report here is posted through the API with a fixed time, so the
//! thresholds compute real boundaries and alerts from a known history.

use bencher_api_tests::TestServer;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, DateTimeMillis, JsonReport, MeasureUuid, ModelTest,
    ProjectSlug, ProjectUuid, TestbedUuid,
    project::{
        Visibility,
        alert::AlertStatus,
        boundary::BoundaryLimit,
        console::{JsonConsolePerf, JsonConsolePerfLine, MAX_CONSOLE_PLOT_LINES},
    },
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// The server's frozen clock.
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
        .signup("Test User", &format!("consoleperf{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Console Perf Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Console Perf Project {label}"))
        .await;
    Fixture {
        slug: project.slug,
        uuid: project.uuid,
        token: user.token,
    }
}

/// One variant of one benchmark and the measures it reports.
fn variant(
    benchmark: &str,
    parameters: serde_json::Value,
    measures: serde_json::Value,
) -> (String, serde_json::Value) {
    let mut entry = serde_json::Map::new();
    entry.insert("parameters".to_owned(), parameters);
    entry.insert("measures".to_owned(), measures);
    (benchmark.to_owned(), serde_json::Value::Object(entry))
}

/// A BMF v1 payload of the given variants, grouped by benchmark in order.
fn results(variants: &[(String, serde_json::Value)]) -> String {
    let mut payload = serde_json::Map::new();
    for (benchmark, entry) in variants {
        payload
            .entry(benchmark.clone())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .expect("an array")
            .push(entry.clone());
    }
    serde_json::Value::Object(payload).to_string()
}

struct Post<'a> {
    time: &'a str,
    branch: &'a str,
    testbed: &'a str,
    hash: String,
    results: String,
    thresholds: Option<serde_json::Value>,
}

impl<'a> Post<'a> {
    fn main(time: &'a str, results: String) -> Self {
        Self {
            time,
            branch: "main",
            testbed: "localhost",
            hash: format!("{:0>40x}", at(time).timestamp()),
            results,
            thresholds: None,
        }
    }
}

async fn post(server: &TestServer, fixture: &Fixture, post: Post<'_>) -> JsonReport {
    let Post {
        time,
        branch,
        testbed,
        hash,
        results,
        thresholds,
    } = post;
    let start_time = at(time);
    let end_time = DateTime::try_from(start_time.timestamp() + 60).expect("Failed to end the run");
    let body = serde_json::json!({
        "branch": branch,
        "testbed": testbed,
        "hash": hash,
        "start_time": start_time,
        "end_time": end_time,
        "results": [results],
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

fn branch(report: &JsonReport) -> BranchUuid {
    report.branch.uuid
}

fn testbed(report: &JsonReport) -> TestbedUuid {
    report.testbed.uuid
}

fn benchmark(report: &JsonReport, name: &str) -> BenchmarkUuid {
    report
        .results
        .as_ref()
        .expect("results")
        .iter()
        .flatten()
        .find(|result| result.benchmark.name.as_ref() == name)
        .map(|result| result.benchmark.uuid)
        .expect("the report has the benchmark")
}

fn measure(report: &JsonReport, slug: &str) -> MeasureUuid {
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

fn list<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// The boxes of a plot query, spelled as its query string.
#[derive(Default)]
struct Query {
    branches: Vec<BranchUuid>,
    testbeds: Vec<TestbedUuid>,
    benchmarks: Vec<BenchmarkUuid>,
    measures: Vec<MeasureUuid>,
    heads: Option<String>,
    specs: Option<String>,
    metrics: Option<Vec<&'static str>>,
    parameters: Option<&'static str>,
    start_time: Option<&'static str>,
    end_time: Option<&'static str>,
}

impl Query {
    fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            ("branches", list(&self.branches)),
            ("testbeds", list(&self.testbeds)),
            ("benchmarks", list(&self.benchmarks)),
            ("measures", list(&self.measures)),
        ];
        if let Some(heads) = &self.heads {
            pairs.push(("heads", heads.clone()));
        }
        if let Some(specs) = &self.specs {
            pairs.push(("specs", specs.clone()));
        }
        if let Some(metrics) = &self.metrics {
            pairs.push(("metrics", metrics.join(",")));
        }
        if let Some(parameters) = self.parameters {
            pairs.push(("parameters", parameters.to_owned()));
        }
        if let Some(start_time) = self.start_time {
            pairs.push(("start_time", millis(start_time).to_string()));
        }
        if let Some(end_time) = self.end_time {
            pairs.push(("end_time", millis(end_time).to_string()));
        }
        pairs
    }
}

async fn try_console_perf(
    server: &TestServer,
    project: &ProjectSlug,
    query: &Query,
    token: Option<&str>,
) -> (StatusCode, String) {
    let mut request = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project}/console/perf")))
        .query(&query.pairs());
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

async fn console_perf(server: &TestServer, fixture: &Fixture, query: &Query) -> JsonConsolePerf {
    let (status, text) = try_console_perf(server, &fixture.slug, query, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "GET console perf: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console perf")
}

/// A line as its key names it.
fn name(perf: &JsonConsolePerf, line: &JsonConsolePerfLine) -> String {
    format!(
        "{} {} {} {} {} {}",
        perf.branches[line.branch as usize].name,
        perf.testbeds[line.testbed as usize].name,
        perf.benchmarks[line.benchmark as usize].name,
        perf.variants[line.variant as usize].parameters,
        perf.measures[line.measure as usize].name,
        line.metric,
    )
}

fn names(perf: &JsonConsolePerf) -> Vec<String> {
    perf.lines.iter().map(|line| name(perf, line)).collect()
}

fn x(perf: &JsonConsolePerf) -> Vec<i64> {
    perf.points.x.iter().copied().map(i64::from).collect()
}

fn latency(value: f64) -> serde_json::Value {
    serde_json::json!({ "latency": { "value": value } })
}

/// Two variants of `alpha`, the larger one missing from the middle report.
async fn two_variants(server: &TestServer, fixture: &Fixture) -> Vec<JsonReport> {
    let mut reports = Vec::new();
    for (time, small, large) in [
        ("2024-02-10T00:00:00Z", 100.0, Some(200.0)),
        ("2024-02-15T00:00:00Z", 101.0, None),
        ("2024-02-20T00:00:00Z", 102.0, Some(202.0)),
    ] {
        let mut variants = vec![variant(
            "alpha",
            serde_json::json!({ "size": 2 }),
            latency(small),
        )];
        if let Some(large) = large {
            variants.push(variant(
                "alpha",
                serde_json::json!({ "size": 10 }),
                latency(large),
            ));
        }
        reports.push(post(server, fixture, Post::main(time, results(&variants))).await);
    }
    reports
}

fn alpha_latency(report: &JsonReport) -> Query {
    Query {
        branches: vec![branch(report)],
        testbeds: vec![testbed(report)],
        benchmarks: vec![benchmark(report, "alpha")],
        measures: vec![measure(report, "latency")],
        metrics: Some(vec!["value"]),
        ..Query::default()
    }
}

// Kills: points that are not shared across lines, a missing point that is
// dropped instead of drawn as null, and a report table out of step with x.
#[tokio::test]
async fn console_perf_columns_share_one_x() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "columns").await;
    let reports = two_variants(&server, &fixture).await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&reports[0])
        },
    )
    .await;

    assert_eq!(perf.total, 2);
    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main localhost alpha {"size":10} Latency value"#,
        ]
    );
    assert_eq!(
        x(&perf),
        vec![
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-15T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
        ]
    );
    assert_eq!(perf.points.report, vec![0, 1, 2]);
    assert!(perf.points.iteration.is_none());
    assert_eq!(
        perf.reports
            .iter()
            .map(|report| report.uuid)
            .collect::<Vec<_>>(),
        reports.iter().map(|report| report.uuid).collect::<Vec<_>>()
    );
    assert_eq!(
        perf.lines[0].series.y,
        vec![Some(100.0), Some(101.0), Some(102.0)]
    );
    assert_eq!(perf.lines[1].series.y, vec![Some(200.0), None, Some(202.0)]);
    assert_eq!(
        i64::from(perf.window.start_time),
        millis("2024-02-01T00:00:00Z")
    );
    assert_eq!(i64::from(perf.window.end_time), millis(NOW));
    assert!(!perf.window.clamped);
}

// Kills: lines ordered by name instead of the boxes' order, a nesting other than
// branch, testbed, benchmark, variant, measure, metric, and lines with no point
// left out of the product.
#[tokio::test]
async fn console_perf_lines_follow_the_boxes() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "order").await;
    let payload = results(&[
        variant(
            "alpha",
            serde_json::json!({ "size": 2 }),
            serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 }, "throughput": { "value": 3.0 } }),
        ),
        variant(
            "alpha",
            serde_json::json!({ "size": 10 }),
            serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 }, "throughput": { "value": 3.0 } }),
        ),
        variant(
            "beta",
            serde_json::json!({ "size": 1 }),
            serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 }, "throughput": { "value": 3.0 } }),
        ),
    ]);
    let main = post(
        &server,
        &fixture,
        Post::main("2024-02-10T00:00:00Z", payload.clone()),
    )
    .await;
    let elsewhere = post(
        &server,
        &fixture,
        Post {
            testbed: "elsewhere",
            ..Post::main("2024-02-11T00:00:00Z", payload.clone())
        },
    )
    .await;
    let feature = post(
        &server,
        &fixture,
        Post {
            branch: "feature",
            ..Post::main("2024-02-12T00:00:00Z", payload)
        },
    )
    .await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![branch(&feature), branch(&main)],
            testbeds: vec![testbed(&elsewhere), testbed(&main)],
            benchmarks: vec![benchmark(&main, "beta"), benchmark(&main, "alpha")],
            measures: vec![measure(&main, "throughput"), measure(&main, "latency")],
            metrics: Some(vec!["value", "p99"]),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..Query::default()
        },
    )
    .await;

    // 2 branches, 2 testbeds, 3 variants, 2 measures, and 2 metric names.
    assert_eq!(perf.total, 48);
    assert_eq!(perf.lines.len(), 48);
    assert_eq!(
        names(&perf)[..13].to_vec(),
        vec![
            r#"feature elsewhere beta {"size":1} Throughput value"#,
            r#"feature elsewhere beta {"size":1} Throughput p99"#,
            r#"feature elsewhere beta {"size":1} Latency value"#,
            r#"feature elsewhere beta {"size":1} Latency p99"#,
            r#"feature elsewhere alpha {"size":2} Throughput value"#,
            r#"feature elsewhere alpha {"size":2} Throughput p99"#,
            r#"feature elsewhere alpha {"size":2} Latency value"#,
            r#"feature elsewhere alpha {"size":2} Latency p99"#,
            r#"feature elsewhere alpha {"size":10} Throughput value"#,
            r#"feature elsewhere alpha {"size":10} Throughput p99"#,
            r#"feature elsewhere alpha {"size":10} Latency value"#,
            r#"feature elsewhere alpha {"size":10} Latency p99"#,
            r#"feature localhost beta {"size":1} Throughput value"#,
        ]
    );
    assert_eq!(
        names(&perf)[47],
        r#"main localhost alpha {"size":10} Latency p99"#
    );
    assert!(
        perf.lines[0].series.y.iter().all(Option::is_none),
        "nothing ran on feature at elsewhere"
    );
    let feature_here = perf
        .lines
        .iter()
        .find(|line| name(&perf, line) == r#"feature localhost beta {"size":1} Latency p99"#)
        .expect("the line");
    assert!(feature_here.series.y.contains(&Some(2.0)));

    // The boxes' order wins whichever way the measures were created.
    let reversed = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![branch(&main)],
            testbeds: vec![testbed(&main)],
            benchmarks: vec![benchmark(&main, "beta")],
            measures: vec![measure(&main, "latency"), measure(&main, "throughput")],
            metrics: Some(vec!["value"]),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..Query::default()
        },
    )
    .await;
    assert_eq!(
        names(&reversed),
        vec![
            r#"main localhost beta {"size":1} Latency value"#,
            r#"main localhost beta {"size":1} Throughput value"#,
        ]
    );
}

// Kills: no line cap, a total that counts what was drawn rather than the
// product of the boxes, and a cut that is not in the boxes' order.
#[tokio::test]
async fn console_perf_cap_counts_the_product() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "cap").await;
    // Seven names do not divide the cap, so the cut falls inside a variant.
    let names_of = ["value", "p1", "p2", "p3", "p4", "p5", "p6"];
    let measures = serde_json::json!({
        "latency": names_of.iter().map(|name| ((*name).to_owned(), serde_json::json!(1.0))).collect::<serde_json::Map<_, _>>(),
    });
    let variants = (0..11)
        .map(|index| {
            variant(
                "wide",
                serde_json::json!({ "index": index }),
                measures.clone(),
            )
        })
        .collect::<Vec<_>>();
    let report = post(
        &server,
        &fixture,
        Post::main("2024-02-10T00:00:00Z", results(&variants)),
    )
    .await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![branch(&report)],
            testbeds: vec![testbed(&report)],
            benchmarks: vec![benchmark(&report, "wide")],
            measures: vec![measure(&report, "latency")],
            metrics: Some(names_of.to_vec()),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..Query::default()
        },
    )
    .await;

    assert_eq!(perf.total, 77, "11 variants by 7 metric names");
    assert_eq!(perf.lines.len(), MAX_CONSOLE_PLOT_LINES);
    assert_eq!(
        names(&perf).last().map(String::as_str),
        Some(r#"main localhost wide {"index":9} Latency value"#)
    );
    assert_eq!(
        perf.lines.last().map(|line| line.series.y.clone()),
        Some(vec![Some(1.0)]),
        "the series the cut falls inside is read"
    );
}

// Kills: an absent metrics box that draws only `value`, names that do not come
// from the window or from the query's branches, testbeds, and variants, and a
// measure drawn with a name only another measure reports.
#[tokio::test]
async fn console_perf_absent_metrics_draw_every_name() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "names").await;
    post(
        &server,
        &fixture,
        Post::main(
            "2024-01-10T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0 }, "throughput": { "value": 3.0, "p50": 4.0 } }),
            )]),
        ),
    )
    .await;
    let report = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 }, "throughput": { "value": 3.0 } }),
            )]),
        ),
    )
    .await;
    // Names that only runs outside the query report.
    for (post_with, payload) in [
        (
            Post {
                branch: "feature",
                ..Post::main("2024-02-11T00:00:00Z", String::new())
            },
            variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p90": 2.0 } }),
            ),
        ),
        (
            Post {
                testbed: "elsewhere",
                ..Post::main("2024-02-12T00:00:00Z", String::new())
            },
            variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p95": 2.0 } }),
            ),
        ),
        (
            Post::main("2024-02-13T00:00:00Z", String::new()),
            variant(
                "beta",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p75": 2.0 } }),
            ),
        ),
        (
            Post::main("2024-02-25T00:00:00Z", String::new()),
            variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "throughput": { "value": 3.0, "p25": 4.0 } }),
            ),
        ),
    ] {
        post(
            &server,
            &fixture,
            Post {
                results: results(&[payload]),
                ..post_with
            },
        )
        .await;
    }

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            metrics: None,
            measures: vec![measure(&report, "latency"), measure(&report, "throughput")],
            start_time: Some("2024-02-01T00:00:00Z"),
            end_time: Some("2024-02-20T00:00:00Z"),
            ..alpha_latency(&report)
        },
    )
    .await;

    assert_eq!(perf.total, 3);
    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency p99"#,
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main localhost alpha {"size":2} Throughput value"#,
        ]
    );
    assert_eq!(perf.lines[2].series.y, vec![Some(3.0)]);
}

// Kills: names gathered only from the series that are read, which undercounts
// the total when a name first appears past the drawn lines, and names gathered
// per measure, which draws a name for every variant when one reported it.
#[tokio::test]
async fn console_perf_absent_metrics_count_names_past_the_cap() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "pastcap").await;
    let variants = (0..71)
        .map(|index| {
            let measures = if index == 70 {
                serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 } })
            } else {
                latency(1.0)
            };
            variant("wide", serde_json::json!({ "index": index }), measures)
        })
        .collect::<Vec<_>>();
    let report = post(
        &server,
        &fixture,
        Post::main("2024-02-10T00:00:00Z", results(&variants)),
    )
    .await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![branch(&report)],
            testbeds: vec![testbed(&report)],
            benchmarks: vec![benchmark(&report, "wide")],
            measures: vec![measure(&report, "latency")],
            start_time: Some("2024-02-01T00:00:00Z"),
            ..Query::default()
        },
    )
    .await;

    assert_eq!(
        perf.total, 72,
        "71 variants with value, and p99 for the one that reported it"
    );
    assert_eq!(perf.lines.len(), MAX_CONSOLE_PLOT_LINES);
    assert_eq!(
        names(&perf)[..2].to_vec(),
        vec![
            r#"main localhost wide {"index":0} Latency value"#,
            r#"main localhost wide {"index":1} Latency value"#,
        ]
    );
}

// Kills: names gathered across the query's branches or testbeds, which draws an
// empty line on a branch or testbed that never reported the name.
#[tokio::test]
async fn console_perf_absent_metrics_are_per_series() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "perseries").await;
    let size_2 = || serde_json::json!({ "size": 2 });
    let report = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:00Z",
            results(&[variant(
                "alpha",
                size_2(),
                serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 } }),
            )]),
        ),
    )
    .await;
    let feature = post(
        &server,
        &fixture,
        Post {
            branch: "feature",
            ..Post::main(
                "2024-02-11T00:00:00Z",
                results(&[variant("alpha", size_2(), latency(1.0))]),
            )
        },
    )
    .await;
    let elsewhere = post(
        &server,
        &fixture,
        Post {
            testbed: "elsewhere",
            ..Post::main(
                "2024-02-12T00:00:00Z",
                results(&[variant("alpha", size_2(), latency(1.0))]),
            )
        },
    )
    .await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![branch(&report), branch(&feature)],
            testbeds: vec![testbed(&report), testbed(&elsewhere)],
            metrics: None,
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&report)
        },
    )
    .await;

    assert_eq!(perf.total, 4);
    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency p99"#,
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main elsewhere alpha {"size":2} Latency value"#,
            r#"feature localhost alpha {"size":2} Latency value"#,
        ]
    );
}

// Kills: a series without its limits, baseline, or alert, and a model that is
// not referenced from its line.
#[tokio::test]
async fn console_perf_series_carry_limits_and_alerts() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "limits").await;
    let thresholds = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.25 },
    }]});
    let mut last = None;
    for (time, value) in [
        ("2024-02-10T00:00:00Z", 200.0),
        ("2024-02-11T00:00:00Z", 200.0),
        ("2024-02-12T00:00:00Z", 200.0),
        ("2024-02-13T00:00:00Z", 260.0),
    ] {
        let payload = results(&[variant(
            "alpha",
            serde_json::json!({ "size": 10 }),
            serde_json::json!({ "latency": { "value": value }, "throughput": { "value": 3.0 } }),
        )]);
        last = Some(
            post(
                &server,
                &fixture,
                Post {
                    thresholds: Some(thresholds.clone()),
                    ..Post::main(time, payload)
                },
            )
            .await,
        );
    }
    let last = last.expect("a report");

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            measures: vec![measure(&last, "latency"), measure(&last, "throughput")],
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&last)
        },
    )
    .await;

    let checked = &perf.lines[0];
    assert_eq!(
        checked.series.y,
        vec![Some(200.0), Some(200.0), Some(200.0), Some(260.0)]
    );
    assert_eq!(
        checked.series.baseline,
        Some(vec![None, Some(200.0), Some(200.0), Some(200.0)])
    );
    assert_eq!(
        checked.series.upper,
        Some(vec![None, Some(250.0), Some(250.0), Some(250.0)])
    );
    assert!(checked.series.lower.is_none());
    let alert = last
        .alerts
        .as_ref()
        .and_then(|alerts| alerts.first())
        .expect("the spike alerted");
    assert_eq!(checked.series.alerts.len(), 1);
    assert_eq!(checked.series.alerts[0].index, 3);
    assert_eq!(checked.series.alerts[0].uuid, alert.uuid);
    assert!(matches!(
        checked.series.alerts[0].limit,
        BoundaryLimit::Upper
    ));
    assert!(matches!(
        checked.series.alerts[0].status,
        AlertStatus::Active
    ));
    let model = &perf.models[checked.model.expect("a threshold checked it") as usize];
    assert_eq!(model.threshold, alert.threshold.uuid);
    assert!(matches!(model.test, ModelTest::Percentage));
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.25));

    let unchecked = &perf.lines[1];
    assert!(unchecked.model.is_none());
    assert!(unchecked.series.baseline.is_none());
    assert!(unchecked.series.upper.is_none());
    assert!(unchecked.series.alerts.is_empty());
    assert_eq!(perf.models.len(), 1);
}

// Kills: an end that does not bound the window, a default start other than four
// weeks before the end, and an unauthenticated window that reaches back past
// three months or is not echoed.
#[tokio::test]
async fn console_perf_window_and_clamp() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "window").await;
    let old = post(
        &server,
        &fixture,
        Post::main(
            "2023-11-01T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                latency(99.0),
            )]),
        ),
    )
    .await;
    two_variants(&server, &fixture).await;

    let ended = console_perf(
        &server,
        &fixture,
        &Query {
            end_time: Some("2024-02-15T00:01:00Z"),
            ..alpha_latency(&old)
        },
    )
    .await;
    assert_eq!(
        x(&ended),
        vec![
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-15T00:00:00Z"),
        ]
    );
    assert_eq!(
        i64::from(ended.window.start_time),
        millis("2024-01-18T00:01:00Z"),
        "four weeks before the end"
    );

    let long_ago = Query {
        start_time: Some("2023-10-01T00:00:00Z"),
        ..alpha_latency(&old)
    };
    let all = console_perf(&server, &fixture, &long_ago).await;
    assert!(!all.window.clamped);
    assert_eq!(x(&all).len(), 4);

    let (status, text) = try_console_perf(&server, &fixture.slug, &long_ago, None).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let clamped: JsonConsolePerf = serde_json::from_str(&text).expect("Failed to parse");
    assert!(clamped.window.clamped);
    assert_eq!(
        i64::from(clamped.window.start_time),
        millis("2023-11-30T00:00:00Z"),
        "92 days before now"
    );
    assert_eq!(
        x(&clamped).len(),
        3,
        "the report from November is out of reach"
    );
}

// Kills: a start after the end that is served rather than refused, and an
// unauthenticated window wholly out of reach echoed inverted rather than empty.
#[tokio::test]
async fn console_perf_window_edges() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "edges").await;
    let old = post(
        &server,
        &fixture,
        Post::main(
            "2023-11-01T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                latency(99.0),
            )]),
        ),
    )
    .await;

    let inverted = Query {
        start_time: Some("2024-02-20T00:00:00Z"),
        end_time: Some("2024-02-10T00:00:00Z"),
        ..alpha_latency(&old)
    };
    let (status, text) =
        try_console_perf(&server, &fixture.slug, &inverted, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");

    let out_of_reach = Query {
        start_time: Some("2023-10-01T00:00:00Z"),
        end_time: Some("2023-11-15T00:00:00Z"),
        ..alpha_latency(&old)
    };
    let (status, text) = try_console_perf(&server, &fixture.slug, &out_of_reach, None).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let empty: JsonConsolePerf = serde_json::from_str(&text).expect("Failed to parse");
    assert!(empty.window.clamped);
    assert_eq!(
        i64::from(empty.window.start_time),
        millis("2023-11-15T00:00:00Z"),
        "an empty window at its end"
    );
    assert_eq!(
        i64::from(empty.window.end_time),
        millis("2023-11-15T00:00:00Z")
    );
    assert_eq!(x(&empty), Vec::<i64>::new());
    assert_eq!(empty.total, 1, "the queried line is still drawn");
    assert_eq!(empty.lines[0].series.y, Vec::new());
}

// Kills: a box entry named twice that draws its lines twice or counts them twice.
#[tokio::test]
async fn console_perf_duplicate_boxes_draw_once() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "duplicates").await;
    let reports = two_variants(&server, &fixture).await;
    let query = alpha_latency(&reports[0]);

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![query.branches[0], query.branches[0]],
            testbeds: vec![query.testbeds[0], query.testbeds[0]],
            benchmarks: vec![query.benchmarks[0], query.benchmarks[0]],
            measures: vec![query.measures[0], query.measures[0]],
            metrics: Some(vec!["value", "value"]),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..Query::default()
        },
    )
    .await;

    assert_eq!(perf.total, 2);
    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main localhost alpha {"size":10} Latency value"#,
        ]
    );
}

// Kills: a window that takes in the points or the metric names of a run which
// started before it and ended inside it.
#[tokio::test]
async fn console_perf_run_straddling_the_start_is_outside() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "straddle").await;
    let reports = two_variants(&server, &fixture).await;
    post(
        &server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:20Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p99": 2.0 } }),
            )]),
        ),
    )
    .await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            metrics: None,
            start_time: Some("2024-02-10T00:00:30Z"),
            ..alpha_latency(&reports[0])
        },
    )
    .await;

    assert_eq!(
        x(&perf),
        vec![
            millis("2024-02-15T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
        ]
    );
    assert_eq!(perf.total, 2, "no p99 line");
    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main localhost alpha {"size":10} Latency value"#,
        ]
    );
}

// Kills: a head of another branch read under the branch the query names.
#[tokio::test]
async fn console_perf_head_of_another_branch_draws_nothing() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "heads").await;
    let payload = || {
        results(&[variant(
            "alpha",
            serde_json::json!({ "size": 2 }),
            latency(1.0),
        )])
    };
    let main = post(
        &server,
        &fixture,
        Post::main("2024-02-10T00:00:00Z", payload()),
    )
    .await;
    let feature = post(
        &server,
        &fixture,
        Post {
            branch: "feature",
            ..Post::main("2024-02-11T00:00:00Z", payload())
        },
    )
    .await;
    let on_main = |head: &JsonReport| Query {
        heads: Some(head.branch.head.uuid.to_string()),
        start_time: Some("2024-02-01T00:00:00Z"),
        ..alpha_latency(&main)
    };

    let own = console_perf(&server, &fixture, &on_main(&main)).await;
    assert_eq!(x(&own), vec![millis("2024-02-10T00:00:00Z")]);

    let other = console_perf(&server, &fixture, &on_main(&feature)).await;
    assert_eq!(other.total, 0);
    assert!(other.lines.is_empty());
}

// Kills: a parameters filter that is ignored, or that counts unmatched variants
// in the product.
#[tokio::test]
async fn console_perf_parameters_filter_the_variants() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "parameters").await;
    let reports = two_variants(&server, &fixture).await;

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            parameters: Some(r#"{"size":10}"#),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&reports[0])
        },
    )
    .await;

    assert_eq!(perf.total, 1);
    assert_eq!(
        names(&perf),
        vec![r#"main localhost alpha {"size":10} Latency value"#]
    );
}

// Kills: a query that fails, rather than drops, a benchmark that does not exist,
// and a benchmark of another project read into this one.
#[tokio::test]
async fn console_perf_unknown_benchmark_is_dropped() {
    let server = TestServer::new_at(at(NOW)).await;
    let theirs = fixture(&server, "theirs").await;
    let fixture = fixture(&server, "unknown").await;
    let reports = two_variants(&server, &fixture).await;
    let their_reports = two_variants(&server, &theirs).await;
    let query = alpha_latency(&reports[0]);

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            benchmarks: vec![
                BenchmarkUuid::new(),
                benchmark(&their_reports[0], "alpha"),
                query.benchmarks[0],
            ],
            start_time: Some("2024-02-01T00:00:00Z"),
            ..query
        },
    )
    .await;

    assert_eq!(perf.total, 2);
    assert_eq!(perf.benchmarks.len(), 1);
}

// Kills: a private project served without `view`.
#[tokio::test]
async fn console_perf_private_project_needs_view() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "private").await;
    let reports = two_variants(&server, &fixture).await;
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(fixture.uuid)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut conn)
        .expect("Failed to make the project private");
    drop(conn);
    let query = alpha_latency(&reports[0]);

    let (status, _) = try_console_perf(&server, &fixture.slug, &query, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, text) =
        try_console_perf(&server, &fixture.slug, &query, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
}

// Kills: a line that follows the oldest threshold when a later one raised the
// alert, and limits taken from a threshold the line does not follow.
#[tokio::test]
async fn console_perf_alerted_threshold_wins() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "two").await;
    // The bare threshold comes first and is looser than the filtered one.
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
    let mut last = None;
    for (time, value) in [
        ("2024-02-10T00:00:00Z", 200.0),
        ("2024-02-11T00:00:00Z", 200.0),
        ("2024-02-12T00:00:00Z", 260.0),
    ] {
        let payload = results(&[variant(
            "alpha",
            serde_json::json!({ "size": 10 }),
            latency(value),
        )]);
        last = Some(
            post(
                &server,
                &fixture,
                Post {
                    thresholds: Some(thresholds.clone()),
                    ..Post::main(time, payload)
                },
            )
            .await,
        );
    }
    let last = last.expect("a report");

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&last)
        },
    )
    .await;

    let line = &perf.lines[0];
    assert_eq!(
        line.series.upper,
        Some(vec![None, Some(250.0), Some(250.0)])
    );
    assert_eq!(line.series.alerts.len(), 1);
    let model = &perf.models[line.model.expect("checked") as usize];
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.25));
}

// Kills: a line that follows a threshold from an earlier point rather than the
// one that checked its latest point.
#[tokio::test]
async fn console_perf_line_follows_its_latest_threshold() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "latest").await;
    let bare = serde_json::json!({ "models": [{
        "measure": "latency",
        "metric": "value",
        "model": { "test": "percentage", "upper_boundary": 0.5 },
    }]});
    // The second set replaces the first: the bare threshold stops checking.
    let filtered = serde_json::json!({
        "models": [{
            "parameters": [{ "size": 10 }],
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.25 },
        }],
        "reset": true,
    });
    let mut last = None;
    for (time, thresholds) in [
        ("2024-02-10T00:00:00Z", &bare),
        ("2024-02-11T00:00:00Z", &bare),
        ("2024-02-12T00:00:00Z", &filtered),
        ("2024-02-13T00:00:00Z", &filtered),
    ] {
        let payload = results(&[variant(
            "alpha",
            serde_json::json!({ "size": 10 }),
            latency(200.0),
        )]);
        last = Some(
            post(
                &server,
                &fixture,
                Post {
                    thresholds: Some(thresholds.clone()),
                    ..Post::main(time, payload)
                },
            )
            .await,
        );
    }
    let last = last.expect("a report");

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&last)
        },
    )
    .await;

    let line = &perf.lines[0];
    assert_eq!(
        line.series.upper,
        Some(vec![None, None, Some(250.0), Some(250.0)]),
        "the bare threshold's limit on the second point is not the line's"
    );
    let model = &perf.models[line.model.expect("checked") as usize];
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.25));
}

// Kills: a line that follows the newest of the thresholds that checked its latest
// point quietly, rather than the oldest.
#[tokio::test]
async fn console_perf_quiet_thresholds_follow_the_oldest() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "quiet").await;
    // The bare threshold is created first, so it is the oldest.
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
    let mut last = None;
    for time in [
        "2024-02-10T00:00:00Z",
        "2024-02-11T00:00:00Z",
        "2024-02-12T00:00:00Z",
    ] {
        let payload = results(&[variant(
            "alpha",
            serde_json::json!({ "size": 10 }),
            latency(200.0),
        )]);
        last = Some(
            post(
                &server,
                &fixture,
                Post {
                    thresholds: Some(thresholds.clone()),
                    ..Post::main(time, payload)
                },
            )
            .await,
        );
    }
    let last = last.expect("a report");

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&last)
        },
    )
    .await;

    let line = &perf.lines[0];
    assert!(line.series.alerts.is_empty());
    assert_eq!(
        line.series.upper,
        Some(vec![None, Some(300.0), Some(300.0)])
    );
    let model = &perf.models[line.model.expect("checked") as usize];
    assert_eq!(model.upper_boundary.map(f64::from), Some(0.5));
}

// Kills: a line that draws runs on another spec of its testbed, names taken from
// runs on another spec, and a testbed entry that does not name its spec.
#[cfg(feature = "plus")]
#[tokio::test]
async fn console_perf_spec_draws_only_its_runs() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "spec").await;
    let elsewhere = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 1.0, "p50": 2.0 } }),
            )]),
        ),
    )
    .await;
    let on_spec = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-11T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                serde_json::json!({ "latency": { "value": 3.0, "p90": 4.0 } }),
            )]),
        ),
    )
    .await;
    let spec = bencher_json::SpecUuid::new();
    let mut conn = server.db_conn();
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec),
            schema::spec::name.eq("console-spec"),
            schema::spec::slug.eq("console-spec"),
            schema::spec::os.eq("linux"),
            schema::spec::architecture.eq("x86_64"),
            schema::spec::cpu.eq(4),
            schema::spec::memory.eq(0x0002_0000_0000i64),
            schema::spec::disk.eq(0x0005_0000_0000i64),
            schema::spec::network.eq(true),
            schema::spec::created.eq(at(NOW)),
            schema::spec::modified.eq(at(NOW)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the spec");
    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to read the spec");
    diesel::update(schema::report::table.filter(schema::report::uuid.eq(on_spec.uuid)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to put the run on the spec");
    drop(conn);

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            specs: Some(spec.to_string()),
            metrics: None,
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&elsewhere)
        },
    )
    .await;

    assert_eq!(
        names(&perf),
        vec![
            r#"main localhost alpha {"size":2} Latency p90"#,
            r#"main localhost alpha {"size":2} Latency value"#,
        ]
    );
    assert_eq!(x(&perf), vec![millis("2024-02-11T00:00:00Z")]);
    assert_eq!(perf.lines[1].series.y, vec![Some(3.0)]);
    assert_eq!(perf.testbeds[0].spec, Some(spec));

    // The same testbed with and without the spec: one table entry each.
    let query = alpha_latency(&elsewhere);
    let both = console_perf(
        &server,
        &fixture,
        &Query {
            testbeds: vec![query.testbeds[0], query.testbeds[0]],
            specs: Some(format!("{spec},")),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..query
        },
    )
    .await;
    assert_eq!(
        both.testbeds
            .iter()
            .map(|testbed| testbed.spec)
            .collect::<Vec<_>>(),
        vec![Some(spec), None]
    );
    assert_eq!(both.lines[1].testbed, 1);
    assert_eq!(both.lines[1].series.y, vec![Some(1.0), Some(3.0)]);

    // With no metric names, each entry draws only the names of its own runs.
    let names_absent = console_perf(
        &server,
        &fixture,
        &Query {
            testbeds: vec![testbed(&elsewhere), testbed(&elsewhere)],
            specs: Some(format!("{spec},")),
            metrics: None,
            start_time: Some("2024-02-01T00:00:00Z"),
            ..alpha_latency(&elsewhere)
        },
    )
    .await;
    assert_eq!(
        names(&names_absent),
        vec![
            r#"main localhost alpha {"size":2} Latency p90"#,
            r#"main localhost alpha {"size":2} Latency value"#,
            r#"main localhost alpha {"size":2} Latency p50"#,
            r#"main localhost alpha {"size":2} Latency p90"#,
            r#"main localhost alpha {"size":2} Latency value"#,
        ]
    );
}

// Kills: a branch entry that names the branch's current head rather than the
// head its lines read, and one entry for a branch read at two heads.
#[tokio::test]
async fn console_perf_branches_name_the_head_each_line_reads() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "headtable").await;
    let report = post(
        &server,
        &fixture,
        Post::main(
            "2024-02-10T00:00:00Z",
            results(&[variant(
                "alpha",
                serde_json::json!({ "size": 2 }),
                latency(1.0),
            )]),
        ),
    )
    .await;
    let old = report.branch.head.uuid;
    let mut conn = server.db_conn();
    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(report.branch.uuid))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to read the branch");
    let current = bencher_json::HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(current),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(at(NOW)),
        ))
        .execute(&mut conn)
        .expect("Failed to add a head");
    let current_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(current))
        .select(schema::head::id)
        .first(&mut conn)
        .expect("Failed to read the head");
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(current_id))
        .execute(&mut conn)
        .expect("Failed to move the branch to its new head");
    drop(conn);
    let query = alpha_latency(&report);

    let perf = console_perf(
        &server,
        &fixture,
        &Query {
            branches: vec![query.branches[0], query.branches[0]],
            heads: Some(format!("{old},")),
            start_time: Some("2024-02-01T00:00:00Z"),
            ..query
        },
    )
    .await;

    assert_eq!(
        perf.branches
            .iter()
            .map(|branch| branch.head)
            .collect::<Vec<_>>(),
        vec![old, current]
    );
    assert_eq!(
        perf.lines
            .iter()
            .map(|line| (line.branch, line.series.y.clone()))
            .collect::<Vec<_>>(),
        vec![(0, vec![Some(1.0)]), (1, vec![None])]
    );
}
