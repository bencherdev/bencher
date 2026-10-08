#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! A benchmark's newest report for the console:
//! `/v0/projects/{project}/console/benchmarks/{benchmark}/latest`.

use bencher_api_tests::TestServer;
use bencher_json::{
    BenchmarkUuid, DateTime, JsonReport, ProjectSlug, project::console::JsonConsoleLatestReport,
};
use http::StatusCode;

const NOW: &str = "2024-03-01T00:00:00Z";

fn at(time: &str) -> DateTime {
    serde_json::from_value(serde_json::json!(time)).expect("Failed to parse the time")
}

struct Fixture {
    slug: ProjectSlug,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("consolelatest{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Console Latest Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Console Latest Project {label}"))
        .await;
    Fixture {
        slug: project.slug,
        token: user.token,
    }
}

async fn post(
    server: &TestServer,
    fixture: &Fixture,
    time: &str,
    branch: &str,
    testbed: &str,
    hash: &str,
    results: &serde_json::Value,
) -> JsonReport {
    let start_time = at(time);
    let end_time = DateTime::try_from(start_time.timestamp() + 60).expect("Failed to end the run");
    let body = serde_json::json!({
        "branch": branch,
        "testbed": testbed,
        "hash": hash,
        "start_time": start_time,
        "end_time": end_time,
        "results": [results.to_string()],
        "bmf_version": 1,
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

async fn try_latest(
    server: &TestServer,
    project: &ProjectSlug,
    benchmark: BenchmarkUuid,
    token: Option<&str>,
) -> (StatusCode, String) {
    let mut request = server.client.get(server.api_url(&format!(
        "/v0/projects/{project}/console/benchmarks/{benchmark}/latest"
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

fn benchmark(report: &JsonReport, name: &str) -> BenchmarkUuid {
    report
        .results
        .as_ref()
        .expect("The report has no results")
        .iter()
        .flatten()
        .find(|result| result.benchmark.name.as_ref() == name)
        .expect("The report has no such benchmark")
        .benchmark
        .uuid
}

fn latency(value: f64) -> serde_json::Value {
    serde_json::json!({ "latency": { "value": value } })
}

// Kills: an older report, another benchmark's measures, the report's other
// measures read as the benchmark's, or measures out of name order.
#[tokio::test]
async fn console_latest_names_the_newest_report_and_the_benchmark_measures() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "newest").await;
    // Throughput is made first, so the measures' ids run against their names.
    let older = post(
        &server,
        &fixture,
        "2024-02-01T00:00:00Z",
        "main",
        "localhost",
        "1000000000000000000000000000000000000001",
        &serde_json::json!({
            "alpha": [{
                "parameters": { "size": 1 },
                "measures": { "throughput": { "value": 1.0 } },
            }],
        }),
    )
    .await;
    let newer = post(
        &server,
        &fixture,
        "2024-02-10T00:00:00Z",
        "feature",
        "runner",
        "2000000000000000000000000000000000000002",
        &serde_json::json!({
            "alpha": [{
                "parameters": { "size": 1 },
                "measures": {
                    "latency": { "value": 2.0 },
                    "throughput": { "value": 3.0 },
                },
            }],
            "beta": [{
                "parameters": { "size": 1 },
                "measures": { "instructions": { "value": 4.0 } },
            }],
        }),
    )
    .await;
    // Only `beta` runs after it, so `alpha` keeps the report before.
    post(
        &server,
        &fixture,
        "2024-02-20T00:00:00Z",
        "main",
        "localhost",
        "3000000000000000000000000000000000000003",
        &serde_json::json!({
            "beta": [{ "parameters": { "size": 1 }, "measures": latency(5.0) }],
        }),
    )
    .await;

    let alpha = benchmark(&older, "alpha");
    let (status, text) = try_latest(&server, &fixture.slug, alpha, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let latest: JsonConsoleLatestReport =
        serde_json::from_str(&text).expect("Failed to parse the latest report");
    assert_eq!(latest.uuid, newer.uuid);
    assert_eq!(latest.branch.name.as_ref(), "feature");
    assert_eq!(latest.branch.head, newer.branch.head.uuid);
    assert_eq!(latest.testbed.name.as_ref(), "runner");
    assert_eq!(
        latest.version.hash.map(|hash| hash.to_string()).as_deref(),
        Some("2000000000000000000000000000000000000002")
    );
    assert_eq!(
        latest
            .measures
            .iter()
            .map(|measure| measure.slug.to_string())
            .collect::<Vec<_>>(),
        ["latency", "throughput"]
    );
}

// Kills: a reader who is not signed in, or a benchmark from another project.
#[tokio::test]
async fn console_latest_needs_a_signed_in_reader_of_the_project() {
    let server = TestServer::new_at(at(NOW)).await;
    let mine = fixture(&server, "mine").await;
    let theirs = fixture(&server, "theirs").await;
    let report = post(
        &server,
        &theirs,
        "2024-02-01T00:00:00Z",
        "main",
        "localhost",
        "4000000000000000000000000000000000000004",
        &serde_json::json!({
            "alpha": [{ "parameters": { "size": 1 }, "measures": latency(1.0) }],
        }),
    )
    .await;
    let alpha = benchmark(&report, "alpha");

    let (status, _) = try_latest(&server, &theirs.slug, alpha, None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a public project still needs a login"
    );
    let (status, text) = try_latest(&server, &theirs.slug, alpha, Some(&mine.token)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let (status, _) = try_latest(&server, &mine.slug, alpha, Some(&mine.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
