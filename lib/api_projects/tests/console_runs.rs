#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The console's runs: `/v0/console/projects/{project}/runs` and
//! `/v0/console/projects/{project}/runs/{version}`, and a report's run on
//! `/v0/console/projects/{project}/reports/{report}`.
//!
//! A run is every report with one version, which one branch head made. Every
//! report here is posted through the API with a fixed time.

use bencher_api_tests::TestServer;
use bencher_json::{
    BmfVersion, DateTime, DateTimeMillis, JsonProjectKeyCreated, JsonReport, ProjectSlug,
    ReportUuid, VersionUuid,
    project::{
        alert::AlertStatus,
        console::{
            ConsoleAuthorKind, JsonConsoleAlerts, JsonConsoleReport, JsonConsoleRun,
            JsonConsoleRunLink, JsonConsoleRunReportLink, JsonConsoleRunSummary, JsonConsoleRuns,
            MAX_CONSOLE_ALERTS_PER_PAGE,
        },
        report::{Adapter, ParseOutcome},
    },
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _};
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
    token: String,
    user: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("runs{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Runs Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Runs Project {label}"))
        .await;
    Fixture {
        slug: project.slug,
        token: user.token.clone(),
        user: "Test User".to_owned(),
    }
}

/// Seven lines: two measures of `alpha` at two sizes, two metrics of `beta`, and `gamma`.
fn results(latency: f64) -> String {
    serde_json::json!({
        "alpha": [
            {
                "parameters": { "size": 2 },
                "measures": {
                    "latency": { "value": 100.0 },
                    "throughput": { "value": 50.0 },
                },
            },
            {
                "parameters": { "size": 10 },
                "measures": {
                    "latency": { "value": latency },
                    "throughput": { "value": 50.0 },
                },
            },
        ],
        "beta": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": 10.0, "p99": 30.0 } },
        }],
        "gamma": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": 5.0 } },
        }],
    })
    .to_string()
}

/// One line.
fn one_line() -> String {
    serde_json::json!({
        "gamma": [{
            "parameters": { "size": 1 },
            "measures": { "latency": { "value": 5.0 } },
        }],
    })
    .to_string()
}

/// One more `gamma` latency line than a page of alerts holds.
fn many(latency: f64) -> String {
    let variants = (1..=u32::from(MAX_CONSOLE_ALERTS_PER_PAGE) + 1)
        .map(|size| {
            serde_json::json!({
                "parameters": { "size": size },
                "measures": { "latency": { "value": latency } },
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "gamma": variants }).to_string()
}

const STEADY: f64 = 200.0;
/// Thirty percent over a steady history, past the threshold's quarter.
const JUMP: f64 = 260.0;

fn threshold() -> serde_json::Value {
    serde_json::json!({
        "models": [{
            "measure": "latency",
            "metric": "value",
            "model": { "test": "percentage", "upper_boundary": 0.25 },
        }]
    })
}

struct Post {
    time: &'static str,
    branch: &'static str,
    testbed: &'static str,
    hash: Option<&'static str>,
    adapter: &'static str,
    iterations: Vec<String>,
    thresholds: bool,
    seconds: i64,
    /// A project key's secret, else the user's token.
    key: Option<String>,
}

impl Post {
    fn new(time: &'static str, hash: Option<&'static str>) -> Self {
        Self {
            time,
            branch: "main",
            testbed: "localhost",
            hash,
            adapter: "json",
            iterations: vec![results(STEADY)],
            thresholds: true,
            seconds: 60,
            key: None,
        }
    }
}

async fn post(server: &TestServer, fixture: &Fixture, post: Post) -> JsonReport {
    let Post {
        time,
        branch,
        testbed,
        hash,
        adapter,
        iterations,
        thresholds,
        seconds,
        key,
    } = post;
    let start_time = at(time);
    let end_time =
        DateTime::try_from(start_time.timestamp() + seconds).expect("Failed to end the report");
    let mut body = serde_json::json!({
        "branch": branch,
        "testbed": testbed,
        "start_time": start_time,
        "end_time": end_time,
        "results": iterations,
        "bmf_version": 1,
        "settings": { "adapter": adapter },
    });
    if let Some(object) = body.as_object_mut() {
        if let Some(hash) = hash {
            object.insert("hash".to_owned(), hash.into());
        }
        if thresholds {
            object.insert("thresholds".to_owned(), threshold());
        }
    }
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_deref().unwrap_or(&fixture.token)),
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

async fn project_key(server: &TestServer, fixture: &Fixture, name: &str) -> String {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "POST key {name}");
    let key: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse the key");
    key.key.to_string()
}

async fn archive_testbed(server: &TestServer, fixture: &Fixture, testbed: &str) {
    archive(server, fixture, "testbeds", testbed).await;
}

async fn archive(server: &TestServer, fixture: &Fixture, dimension: &str, slug: &str) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}/{dimension}/{slug}", fixture.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({ "archived": true }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "PATCH {dimension} {slug}");
}

async fn get(server: &TestServer, token: Option<&str>, path: &str) -> (StatusCode, String) {
    let mut request = server.client.get(server.api_url(path));
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

async fn runs(server: &TestServer, fixture: &Fixture, query: &str) -> JsonConsoleRuns {
    let (status, text) = get(
        server,
        Some(&fixture.token),
        &format!("/v0/console/projects/{}/runs{query}", fixture.slug),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET runs{query}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the runs")
}

async fn run(
    server: &TestServer,
    fixture: &Fixture,
    version: VersionUuid,
    query: &str,
) -> JsonConsoleRun {
    let (status, text) = get(
        server,
        Some(&fixture.token),
        &format!(
            "/v0/console/projects/{}/runs/{version}{query}",
            fixture.slug
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET run{query}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the run")
}

fn version_of(server: &TestServer, report: &JsonReport) -> VersionUuid {
    schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(schema::report::uuid.eq(report.uuid))
        .select(schema::version::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to read the report's version")
}

/// Each run as its hash, or its version number when it has none.
fn heads(runs: &JsonConsoleRuns) -> Vec<String> {
    runs.runs.iter().map(head).collect()
}

fn head(run: &JsonConsoleRunSummary) -> String {
    run.hash
        .as_ref()
        .map_or_else(|| format!("v{}", run.number), ToString::to_string)
}

fn testbeds(run: &JsonConsoleRunSummary) -> Vec<String> {
    run.testbeds.iter().map(ToString::to_string).collect()
}

const H1: &str = "1000000000000000000000000000000000000001";
const H2: &str = "1000000000000000000000000000000000000002";
const SPLIT: &str = "5000000000000000000000000000000000000005";
const RETIRED: &str = "9000000000000000000000000000000000000009";

/// A steady history on `main`, then a split run: two reports on `localhost`,
/// the second of which alerts, and one between them on `elsewhere` by a project
/// key from another adapter with a second, shorter iteration; then the same hash
/// on `feature`, and a report with no hash.
struct Seeded {
    fixture: Fixture,
    first: JsonReport,
    split: [JsonReport; 3],
    feature: JsonReport,
    no_hash: JsonReport,
}

async fn seeded(server: &TestServer, label: &str) -> Seeded {
    let fixture = fixture(server, label).await;
    let first = post(
        server,
        &fixture,
        Post::new("2024-02-01T00:00:00Z", Some(H1)),
    )
    .await;
    post(
        server,
        &fixture,
        Post::new("2024-02-05T00:00:00Z", Some(H2)),
    )
    .await;
    let key = project_key(server, &fixture, "ci-matrix").await;
    let split_1 = post(
        server,
        &fixture,
        Post::new("2024-02-20T00:00:00Z", Some(SPLIT)),
    )
    .await;
    let split_2 = post(
        server,
        &fixture,
        Post {
            testbed: "elsewhere",
            adapter: "magic",
            iterations: vec![results(STEADY), one_line()],
            thresholds: false,
            seconds: 600,
            key: Some(key),
            ..Post::new("2024-02-20T00:00:05Z", Some(SPLIT))
        },
    )
    .await;
    let split_3 = post(
        server,
        &fixture,
        Post {
            iterations: vec![results(JUMP)],
            ..Post::new("2024-02-20T00:00:10Z", Some(SPLIT))
        },
    )
    .await;
    let feature = post(
        server,
        &fixture,
        Post {
            branch: "feature",
            thresholds: false,
            ..Post::new("2024-02-22T00:00:00Z", Some(SPLIT))
        },
    )
    .await;
    let no_hash = post(server, &fixture, Post::new("2024-02-25T00:00:00Z", None)).await;
    Seeded {
        fixture,
        first,
        split: [split_1, split_2, split_3],
        feature,
        no_hash,
    }
}

// Kills: grouping by report or by hash instead of version, ordering by the
// last report rather than the first, a run's end taken from its last start, a
// count of testbeds or authors taken from the rows named, names out of first
// use order, an author named by the person when a key sent the report, lines
// summed over iterations instead of the most any reported, and alerts counted
// from one report.
#[tokio::test]
async fn console_runs_group_reports_by_version() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "group").await;

    let listed = runs(&server, &seeded.fixture, "").await;

    assert_eq!(listed.total, 5);
    let no_hash = format!(
        "v{}",
        seeded
            .no_hash
            .branch
            .head
            .version
            .as_ref()
            .expect("a report names its version")
            .number
    );
    assert_eq!(heads(&listed), [no_hash.as_str(), SPLIT, SPLIT, H2, H1]);
    assert_eq!(listed.runs[1].branch.name.as_ref(), "feature");

    let split = &listed.runs[2];
    assert_eq!(split.uuid, version_of(&server, &seeded.split[0]));
    assert_eq!(split.branch.name.as_ref(), "main");
    assert_eq!(split.reports, 3);
    assert_eq!(i64::from(split.start_time), millis("2024-02-20T00:00:00Z"));
    // The key's report starts second and ends last.
    assert_eq!(i64::from(split.end_time), millis("2024-02-20T00:10:05Z"));
    assert_eq!(testbeds(split), ["localhost", "elsewhere"]);
    assert_eq!(split.testbed_count, 2);
    assert_eq!(split.adapters, [Adapter::Json, Adapter::Magic]);
    assert_eq!(split.adapter_count, 2);
    let authors = split
        .authors
        .iter()
        .map(|author| (author.kind, author.name.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        authors,
        [
            (ConsoleAuthorKind::User, seeded.fixture.user.as_str()),
            (ConsoleAuthorKind::Key, "ci-matrix"),
        ]
    );
    assert_eq!(split.author_count, 2);
    assert_eq!(split.lines, 21);
    assert_eq!((split.alerts.active, split.alerts.total), (1, 1));

    let first = &listed.runs[4];
    assert_eq!(first.uuid, version_of(&server, &seeded.first));
    assert_eq!(first.reports, 1);
    assert_eq!(first.lines, 7);
    assert_eq!((first.alerts.active, first.alerts.total), (0, 0));

    dismiss(&server, &seeded.fixture, &seeded.split[2]).await;
    let alerting = runs(&server, &seeded.fixture, "?alerts=active").await;
    assert_eq!(alerting.total, 0);
    let after = runs_of(&server, &seeded.fixture).await;
    assert_eq!((after[2].alerts.active, after[2].alerts.total), (0, 1));
}

async fn runs_of_query(server: &TestServer, fixture: &Fixture, query: &str) -> JsonConsoleRuns {
    runs(server, fixture, query).await
}

async fn runs_of(server: &TestServer, fixture: &Fixture) -> Vec<JsonConsoleRunSummary> {
    runs(server, fixture, "").await.runs
}

/// Dismiss every alert the report raised.
async fn dismiss(server: &TestServer, fixture: &Fixture, report: &JsonReport) {
    for alert in report.alerts.as_ref().expect("the report alerted") {
        let resp = server
            .client
            .patch(server.api_url(&format!(
                "/v0/projects/{}/alerts/{}",
                fixture.slug, alert.uuid
            )))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&fixture.token),
            )
            .json(&serde_json::json!({ "status": "dismissed" }))
            .send()
            .await
            .expect("Request failed");
        assert!(resp.status().is_success(), "PATCH alert: {}", resp.status());
    }
}

// Kills: a run without a hash dropped or given another run's hash.
#[tokio::test]
async fn console_runs_without_a_hash() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "nohash").await;

    let runs = runs(&server, &seeded.fixture, "").await;
    let newest = &runs.runs[0];
    assert_eq!(newest.uuid, version_of(&server, &seeded.no_hash));
    assert!(newest.hash.is_none());
    assert_eq!(newest.reports, 1);

    let run = run(&server, &seeded.fixture, newest.uuid, "").await;
    assert!(run.hash.is_none());
    assert_eq!(run.number, newest.number);
    assert_eq!(run.reports[0].uuid, seeded.no_hash.uuid);
}

// Kills: a filter that never reaches the query, a testbed or adapter filter
// that keeps only the matching reports rather than their whole run, a branch
// filter that keeps runs on other branches, and an alerts filter that keeps
// runs with no active alert.
#[tokio::test]
async fn console_runs_filters() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "filters").await;
    let fixture = &seeded.fixture;

    let by_branch = runs(&server, fixture, "?branch=feature").await;
    assert_eq!(heads(&by_branch), [SPLIT]);
    assert_eq!(by_branch.runs[0].uuid, version_of(&server, &seeded.feature));
    assert_eq!(by_branch.total, 1);

    let by_testbed = runs(&server, fixture, "?testbed=elsewhere").await;
    assert_eq!(by_testbed.total, 1);
    assert_eq!(
        by_testbed.runs[0].uuid,
        version_of(&server, &seeded.split[0])
    );
    assert_eq!(by_testbed.runs[0].reports, 3);
    assert_eq!(testbeds(&by_testbed.runs[0]), ["localhost", "elsewhere"]);

    let by_adapter = runs(&server, fixture, "?adapter=magic").await;
    assert_eq!(by_adapter.total, 1);
    assert_eq!(
        by_adapter.runs[0].uuid,
        version_of(&server, &seeded.split[0])
    );
    assert_eq!(by_adapter.runs[0].reports, 3);

    let alerting = runs(&server, fixture, "?alerts=active").await;
    assert_eq!(alerting.total, 1);
    assert_eq!(alerting.runs[0].uuid, version_of(&server, &seeded.split[0]));

    let both = runs(&server, fixture, "?branch=main&testbed=elsewhere").await;
    assert_eq!(both.total, 1);
    let neither = runs(&server, fixture, "?branch=feature&testbed=elsewhere").await;
    assert_eq!(neither.total, 0);
    assert!(neither.runs.is_empty());
    let nowhere = runs(&server, fixture, "?testbed=nowhere").await;
    assert_eq!(nowhere.total, 0);
}

// Kills: a window that reads any report's start rather than the run's first,
// that keeps a run whose first report started before it, or that cuts a run
// to its reports inside it.
#[tokio::test]
async fn console_runs_window_reads_the_first_report() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "window").await;
    let quiet = |time, hash| Post {
        thresholds: false,
        ..Post::new(time, Some(hash))
    };
    // Straddles the window's start: its first report is before it.
    post(&server, &fixture, quiet("2024-01-31T23:00:00Z", H1)).await;
    post(&server, &fixture, quiet("2024-02-01T01:00:00Z", H1)).await;
    // Starts inside the window and ends after it.
    let inside = post(&server, &fixture, quiet("2024-02-10T00:00:00Z", H2)).await;
    post(&server, &fixture, quiet("2024-02-20T00:00:00Z", H2)).await;
    // Starts after it.
    post(&server, &fixture, quiet("2024-02-25T00:00:00Z", SPLIT)).await;

    let window = format!(
        "?start_time={}&end_time={}",
        millis("2024-02-01T00:00:00Z"),
        millis("2024-02-15T00:00:00Z")
    );
    let runs = runs(&server, &fixture, &window).await;

    assert_eq!(runs.total, 1);
    assert_eq!(runs.runs[0].uuid, version_of(&server, &inside));
    assert_eq!(runs.runs[0].reports, 2);
    assert_eq!(
        i64::from(runs.runs[0].end_time),
        millis("2024-02-20T00:01:00Z")
    );

    let since = format!("?start_time={}", millis("2024-02-01T00:00:00Z"));
    let since = runs_heads(&server, &fixture, &since).await;
    assert_eq!(since, [SPLIT, H2]);
}

async fn runs_heads(server: &TestServer, fixture: &Fixture, query: &str) -> Vec<String> {
    heads(&runs(server, fixture, query).await)
}

// Kills: a filter that finds runs by another project's reports, testbeds, or
// alerts once it, rather than the project, picks the runs to read.
#[tokio::test]
async fn console_runs_filters_stay_in_their_project() {
    let server = TestServer::new_at(at(NOW)).await;
    let ours = seeded(&server, "ours").await;
    let theirs = seeded(&server, "theirs").await;
    let window = format!("?start_time={}", millis("2024-02-01T00:00:00Z"));
    for query in [
        "",
        "?branch=feature",
        "?testbed=elsewhere",
        "?adapter=magic",
        "?alerts=active",
        window.as_str(),
    ] {
        let mine = runs(&server, &ours.fixture, query).await;
        let other = runs(&server, &theirs.fixture, query).await;
        assert!(mine.total > 0, "{query}");
        assert_eq!(mine.total, other.total, "{query}");
        assert_eq!(
            usize::try_from(mine.total).ok(),
            Some(mine.runs.len()),
            "{query}"
        );
        let theirs_uuids = other.runs.iter().map(|run| run.uuid).collect::<Vec<_>>();
        assert!(
            mine.runs
                .iter()
                .all(|run| !theirs_uuids.contains(&run.uuid)),
            "{query}"
        );
    }
}

// Kills: paging that cuts before ordering, a page that repeats or skips a run,
// and a total that counts the page rather than every match.
#[tokio::test]
async fn console_runs_paging_and_count() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "paging").await;
    let fixture = &seeded.fixture;

    let all = heads(&runs(&server, fixture, "").await);
    let mut paged = Vec::new();
    for page in 1..=3 {
        let page = runs(&server, fixture, &format!("?per_page=2&page={page}")).await;
        assert_eq!(page.total, 5);
        paged.extend(heads(&page));
    }
    assert_eq!(paged, all);
    let past = runs(&server, fixture, "?per_page=2&page=4").await;
    assert_eq!(past.total, 5);
    assert!(past.runs.is_empty());
}

// Kills: reports on an archived testbed counted in their run, and a run whose
// every report is archived still listed.
#[tokio::test]
async fn console_runs_leave_out_archived_testbeds() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "archived").await;
    let fixture = &seeded.fixture;
    let retired = post(
        &server,
        fixture,
        Post {
            testbed: "retired",
            thresholds: false,
            ..Post::new("2024-02-27T00:00:00Z", Some(RETIRED))
        },
    )
    .await;
    archive_testbed(&server, fixture, "elsewhere").await;
    archive_testbed(&server, fixture, "retired").await;

    let runs = runs(&server, fixture, "").await;
    assert_eq!(runs.total, 5);
    let split = runs
        .runs
        .iter()
        .find(|run| run.uuid == version_of(&server, &seeded.split[0]))
        .expect("the split run is listed");
    assert_eq!(split.reports, 2);
    assert_eq!(testbeds(split), ["localhost"]);
    assert_eq!(split.adapters, [Adapter::Json]);
    assert_eq!(split.lines, 14);

    let on_archived = runs_of_query(&server, fixture, "?testbed=elsewhere").await;
    assert_eq!(on_archived.total, 0);
    assert!(on_archived.runs.is_empty());

    let run = run(&server, fixture, split.uuid, "").await;
    assert_eq!(run.total, 2);
    assert_eq!(run.testbeds, 1);
    assert_eq!(run.reports.len(), 2);

    let (status, _) = get(
        &server,
        Some(&fixture.token),
        &format!(
            "/v0/console/projects/{}/runs/{}",
            fixture.slug,
            version_of(&server, &retired)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Kills: a run that names every testbed however many, and a count taken from
// the names rather than the run.
#[tokio::test]
async fn console_runs_name_at_most_eight_testbeds() {
    const TESTBEDS: [&str; 9] = ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9"];
    const TIMES: [&str; 9] = [
        "2024-02-20T00:00:01Z",
        "2024-02-20T00:00:02Z",
        "2024-02-20T00:00:03Z",
        "2024-02-20T00:00:04Z",
        "2024-02-20T00:00:05Z",
        "2024-02-20T00:00:06Z",
        "2024-02-20T00:00:07Z",
        "2024-02-20T00:00:08Z",
        "2024-02-20T00:00:09Z",
    ];
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "cap").await;
    for (testbed, time) in TESTBEDS.into_iter().zip(TIMES) {
        post(
            &server,
            &fixture,
            Post {
                testbed,
                thresholds: false,
                ..Post::new(time, Some(SPLIT))
            },
        )
        .await;
    }

    let runs = runs_of(&server, &fixture).await;
    assert_eq!(runs.len(), 1);
    assert_eq!(testbeds(&runs[0]), TESTBEDS[..8]);
    assert_eq!(runs[0].testbed_count, 9);
    assert_eq!(runs[0].reports, 9);
}

// Kills: an archived branch's runs counted in the total, which leaves pages
// short of their rows, listed, or still opening as a run.
#[tokio::test]
async fn console_runs_leave_out_archived_branches() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "archivedbranch").await;
    let fixture = &seeded.fixture;
    let all = heads(&runs(&server, fixture, "").await);
    archive(&server, fixture, "branches", "feature").await;

    let listed = runs(&server, fixture, "").await;
    assert_eq!(listed.total, 4);
    let kept = all
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, head)| head.clone())
        .collect::<Vec<_>>();
    assert_eq!(heads(&listed), kept);
    assert!(
        listed
            .runs
            .iter()
            .all(|run| run.uuid != version_of(&server, &seeded.feature))
    );
    let paged = runs(&server, fixture, "?per_page=2&page=2").await;
    assert_eq!(paged.total, 4);
    assert_eq!(heads(&paged), [H2, H1]);

    let (status, _) = get(
        &server,
        Some(&fixture.token),
        &format!(
            "/v0/console/projects/{}/runs/{}",
            fixture.slug,
            version_of(&server, &seeded.feature)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Kills: an empty project answering with an error or a stale total.
#[tokio::test]
async fn console_runs_of_a_project_with_no_runs() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "empty").await;

    let runs = runs(&server, &fixture, "").await;
    assert_eq!(runs.total, 0);
    assert!(runs.runs.is_empty());
}

// Kills: the run's reports out of start order, numbers that restart on each
// page, heading facts taken from the page, a report's lines summed over its
// iterations, and a report's author or alerts read from another report.
#[tokio::test]
async fn console_run_reports_in_start_order() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "run").await;
    let fixture = &seeded.fixture;
    let version = version_of(&server, &seeded.split[0]);

    let run_page = run(&server, fixture, version, "").await;
    assert_eq!(run_page.uuid, version);
    assert_eq!(
        run_page.hash.as_ref().map(ToString::to_string).as_deref(),
        Some(SPLIT)
    );
    assert_eq!(run_page.branch.name.as_ref(), "main");
    assert_eq!(run_page.total, 3);
    assert_eq!(run_page.testbeds, 2);
    assert_eq!(run_page.lines, 21);
    assert_eq!((run_page.alerts.active, run_page.alerts.total), (1, 1));
    assert_eq!(
        i64::from(run_page.start_time),
        millis("2024-02-20T00:00:00Z")
    );
    assert_eq!(i64::from(run_page.end_time), millis("2024-02-20T00:10:05Z"));
    let order = run_page
        .reports
        .iter()
        .map(|report| (report.order, report.uuid))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [
            (1, seeded.split[0].uuid),
            (2, seeded.split[1].uuid),
            (3, seeded.split[2].uuid),
        ]
    );
    let keyed = &run_page.reports[1];
    assert_eq!(keyed.testbed.name.as_ref(), "elsewhere");
    assert_eq!(keyed.adapter, Adapter::Magic);
    assert_eq!(keyed.lines, 7);
    let author = keyed.author.as_ref().expect("the key sent it");
    assert_eq!(
        (author.kind, author.name.as_str()),
        (ConsoleAuthorKind::Key, "ci-matrix")
    );
    assert_eq!(i64::from(keyed.end_time), millis("2024-02-20T00:10:05Z"));
    let alerting = &run_page.reports[2];
    assert_eq!((alerting.alerts.active, alerting.alerts.total), (1, 1));
    assert_eq!(
        run_page.reports[0]
            .author
            .as_ref()
            .map(|author| author.kind),
        Some(ConsoleAuthorKind::User)
    );

    let second = run(&server, fixture, version, "?per_page=2&page=2").await;
    assert_eq!(second.total, 3);
    assert_eq!(second.lines, 21);
    let order = second
        .reports
        .iter()
        .map(|report| (report.order, report.uuid))
        .collect::<Vec<_>>();
    assert_eq!(order, [(3, seeded.split[2].uuid)]);
}

// Kills: a run read from another project, a version that is not a run, and a
// run served without a login.
#[tokio::test]
async fn console_run_not_found_and_signed_out() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "notfound").await;
    let other = fixture(&server, "notfoundother").await;
    let version = version_of(&server, &seeded.split[0]);

    let (status, _) = get(
        &server,
        Some(&other.token),
        &format!("/v0/console/projects/{}/runs/{version}", other.slug),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(
        &server,
        Some(&seeded.fixture.token),
        &format!(
            "/v0/console/projects/{}/runs/{}",
            seeded.fixture.slug,
            VersionUuid::new()
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for path in [
        format!("/v0/console/projects/{}/runs", seeded.fixture.slug),
        format!(
            "/v0/console/projects/{}/runs/{version}",
            seeded.fixture.slug
        ),
    ] {
        let (status, _) = get(&server, None, &path).await;
        assert!(
            matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
            "{path}: {status}"
        );
    }
}

/// A run's Previous or Next as its hash, or its version number when it has none.
fn link_head(link: &JsonConsoleRunLink) -> String {
    link.hash
        .as_ref()
        .map_or_else(|| format!("v{}", link.number), ToString::to_string)
}

/// The run page's Previous and Next for the report's run.
async fn steps(
    server: &TestServer,
    fixture: &Fixture,
    report: &JsonReport,
) -> (Option<String>, Option<String>) {
    let run = run(server, fixture, version_of(server, report), "").await;
    (
        run.previous.as_ref().map(link_head),
        run.next.as_ref().map(link_head),
    )
}

const STEP: [&str; 7] = [
    "a000000000000000000000000000000000000001",
    "a000000000000000000000000000000000000002",
    "a000000000000000000000000000000000000003",
    "a000000000000000000000000000000000000004",
    "a000000000000000000000000000000000000005",
    "a000000000000000000000000000000000000006",
    "a000000000000000000000000000000000000007",
];

fn stepped(index: usize) -> Option<String> {
    STEP.get(index).map(|hash| (*hash).to_owned())
}

// Kills: Previous and Next taken from another branch's runs, from a run whose
// every report is archived, by a run's last report rather than its first, by
// start alone where two runs start together, past either end of the branch,
// and a link that names another run or its start.
#[tokio::test]
async fn console_run_previous_and_next_on_its_branch() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "steps").await;
    let first = post(
        &server,
        &fixture,
        Post::new("2024-02-01T00:00:00Z", Some(STEP[0])),
    )
    .await;
    // Starts second and ends last.
    let late = post(
        &server,
        &fixture,
        Post::new("2024-02-03T00:00:00Z", Some(STEP[1])),
    )
    .await;
    let tied_low = post(
        &server,
        &fixture,
        Post::new("2024-02-05T00:00:00Z", Some(STEP[2])),
    )
    .await;
    let tied_high = post(
        &server,
        &fixture,
        Post::new("2024-02-05T00:00:00Z", Some(STEP[3])),
    )
    .await;
    post(
        &server,
        &fixture,
        Post {
            branch: "feature",
            thresholds: false,
            ..Post::new("2024-02-06T00:00:00Z", Some(STEP[4]))
        },
    )
    .await;
    post(
        &server,
        &fixture,
        Post {
            testbed: "retired",
            thresholds: false,
            ..Post::new("2024-02-07T00:00:00Z", Some(STEP[5]))
        },
    )
    .await;
    let last = post(
        &server,
        &fixture,
        Post::new("2024-02-10T00:00:00Z", Some(STEP[6])),
    )
    .await;
    post(
        &server,
        &fixture,
        Post::new("2024-02-12T00:00:00Z", Some(STEP[1])),
    )
    .await;
    archive_testbed(&server, &fixture, "retired").await;

    assert_eq!(steps(&server, &fixture, &first).await, (None, stepped(1)));
    assert_eq!(
        steps(&server, &fixture, &late).await,
        (stepped(0), stepped(2))
    );
    assert_eq!(
        steps(&server, &fixture, &tied_low).await,
        (stepped(1), stepped(3))
    );
    assert_eq!(
        steps(&server, &fixture, &tied_high).await,
        (stepped(2), stepped(6))
    );
    assert_eq!(steps(&server, &fixture, &last).await, (stepped(3), None));

    let run = run(&server, &fixture, version_of(&server, &last), "").await;
    assert!(
        run.alerting.is_none(),
        "a run that raised no alerts reads none"
    );
    let previous = run.previous.expect("the last run has a previous run");
    assert_eq!(previous.uuid, version_of(&server, &tied_high));
    assert_eq!(
        Some(previous.number),
        tied_high
            .branch
            .head
            .version
            .as_ref()
            .map(|version| version.number)
    );
    assert_eq!(
        i64::from(previous.start_time),
        millis("2024-02-05T00:00:00Z")
    );
}

async fn report_page(
    server: &TestServer,
    fixture: &Fixture,
    report: &JsonReport,
) -> JsonConsoleReport {
    report_page_with(server, fixture, report, "?per_page=1").await
}

async fn report_page_with(
    server: &TestServer,
    fixture: &Fixture,
    report: &JsonReport,
    query: &str,
) -> JsonConsoleReport {
    let (status, text) = get(
        server,
        Some(&fixture.token),
        &format!(
            "/v0/console/projects/{}/reports/{}{query}",
            fixture.slug, report.uuid
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET report: {text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

/// A step through a run: the report's place, the report, and its testbed.
fn step(link: Option<&JsonConsoleRunReportLink>) -> Option<(u32, ReportUuid, String)> {
    link.map(|link| (link.order, link.uuid, link.testbed.to_string()))
}

// Kills: Previous and Next that step by branch and testbed rather than through
// the run, a place in the run that differs from the run page's, a count taken
// from anything but the run's reports, and a step that names another report's
// start.
#[tokio::test]
async fn console_report_steps_through_its_run() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "reportrun").await;
    let fixture = &seeded.fixture;
    let version = version_of(&server, &seeded.split[0]);
    let [one, two, three] = &seeded.split;
    let localhost = || "localhost".to_owned();
    let elsewhere = || "elsewhere".to_owned();
    for (report, order, previous, next) in [
        (one, 1, None, Some((2, two.uuid, elsewhere()))),
        (
            two,
            2,
            Some((1, one.uuid, localhost())),
            Some((3, three.uuid, localhost())),
        ),
        (three, 3, Some((2, two.uuid, elsewhere())), None),
    ] {
        let page = report_page(&server, fixture, report).await;
        let run = page.run.expect("the report is in a run");
        assert_eq!(run.uuid, version);
        assert_eq!(run.order, Some(order));
        assert_eq!(run.total, 3);
        assert_eq!(step(run.previous.as_ref()), previous, "#{order}'s previous");
        assert_eq!(step(run.next.as_ref()), next, "#{order}'s next");
    }
    let middle = report_page(&server, fixture, two).await;
    let next = middle
        .run
        .and_then(|run| run.next)
        .expect("the second report has a next");
    assert_eq!(i64::from(next.start_time), millis("2024-02-20T00:00:10Z"));
    for query in ["?per_page=1&page=2", "?per_page=0"] {
        let page = report_page_with(&server, fixture, two, query).await;
        assert!(page.run.is_none(), "{query} reads no run");
    }
}

// Kills: a run's reports that start together placed by anything before their
// end, on the run page or on the report's.
#[tokio::test]
async fn console_report_places_reports_that_start_together_by_end() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "startedtogether").await;
    let long = post(
        &server,
        &fixture,
        Post {
            seconds: 600,
            ..Post::new("2024-02-20T00:00:00Z", Some(SPLIT))
        },
    )
    .await;
    let short = post(
        &server,
        &fixture,
        Post::new("2024-02-20T00:00:00Z", Some(SPLIT)),
    )
    .await;

    let run_page = run(&server, &fixture, version_of(&server, &long), "").await;
    let order = run_page
        .reports
        .iter()
        .map(|report| (report.order, report.uuid))
        .collect::<Vec<_>>();
    assert_eq!(order, [(1, short.uuid), (2, long.uuid)]);
    let long_page = report_page(&server, &fixture, &long).await;
    let long_run = long_page.run.expect("the long report is in a run");
    assert_eq!(long_run.order, Some(2));
    assert_eq!(long_run.previous.map(|link| link.uuid), Some(short.uuid));
}

// Kills: a report on an archived testbed numbered in its run or counted in it,
// steps that stop at it or skip past its neighbors, and a run named for a
// report the runs list leaves out with its whole run.
#[tokio::test]
async fn console_report_run_leaves_out_archived_reports() {
    let server = TestServer::new_at(at(NOW)).await;
    let seeded = seeded(&server, "reportarchived").await;
    let fixture = &seeded.fixture;
    let [one, two, three] = &seeded.split;
    archive_testbed(&server, fixture, "elsewhere").await;

    let hidden = report_page(&server, fixture, two).await;
    let run = hidden.run.expect("the run still lists its other reports");
    assert_eq!(run.order, None);
    assert_eq!(run.total, 2);
    assert_eq!(
        step(run.previous.as_ref()),
        Some((1, one.uuid, "localhost".to_owned()))
    );
    assert_eq!(
        step(run.next.as_ref()),
        Some((2, three.uuid, "localhost".to_owned()))
    );
    let after = report_page(&server, fixture, three).await;
    let run = after.run.expect("the third report is in its run");
    assert_eq!(run.order, Some(2));
    assert_eq!(
        step(run.previous.as_ref()),
        Some((1, one.uuid, "localhost".to_owned()))
    );

    archive(&server, fixture, "branches", "feature").await;
    let feature = report_page(&server, fixture, &seeded.feature).await;
    assert!(feature.run.is_none(), "the runs list leaves its run out");
}

/// Each group's place in the run and report, with its alerts' lines by name.
fn alert_groups(alerting: &JsonConsoleAlerts) -> Vec<(Option<u32>, ReportUuid, Vec<String>)> {
    alerting
        .groups
        .iter()
        .map(|group| {
            let lines = group
                .alerts
                .iter()
                .map(|alert| {
                    let line = &alert.line;
                    let benchmark = &alerting.benchmarks[line.benchmark as usize];
                    let variant = &alerting.variants[line.variant as usize];
                    let measure = &alerting.measures[line.measure as usize];
                    format!(
                        "{} {} {}",
                        benchmark.name,
                        variant.parameters.tags().collect::<Vec<_>>().join(" "),
                        measure.name
                    )
                })
                .collect();
            (group.order, group.uuid, lines)
        })
        .collect()
}

// Kills: alerts from another run or from a report the run leaves out, groups
// out of start order or numbered by anything but the run's order, a dismissed
// alert dropped or counted as active, and a history read off another testbed.
#[tokio::test]
async fn console_run_alerts_across_its_reports() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "runalerts").await;
    for (time, hash) in [("2024-02-01T00:00:00Z", H1), ("2024-02-05T00:00:00Z", H2)] {
        for testbed in ["localhost", "elsewhere"] {
            post(
                &server,
                &fixture,
                Post {
                    testbed,
                    ..Post::new(time, Some(hash))
                },
            )
            .await;
        }
    }
    let first = post(
        &server,
        &fixture,
        Post {
            iterations: vec![results(JUMP)],
            ..Post::new("2024-02-20T00:00:00Z", Some(SPLIT))
        },
    )
    .await;
    post(
        &server,
        &fixture,
        Post {
            testbed: "elsewhere",
            ..Post::new("2024-02-20T00:00:02Z", Some(SPLIT))
        },
    )
    .await;
    let third = post(
        &server,
        &fixture,
        Post {
            testbed: "elsewhere",
            iterations: vec![results(JUMP)],
            ..Post::new("2024-02-20T00:00:05Z", Some(SPLIT))
        },
    )
    .await;
    let later = post(
        &server,
        &fixture,
        Post {
            iterations: vec![results(2.0 * JUMP)],
            ..Post::new("2024-02-25T00:00:00Z", Some(RETIRED))
        },
    )
    .await;
    assert!(
        later
            .alerts
            .as_ref()
            .is_some_and(|alerts| !alerts.is_empty()),
        "the later run alerts too"
    );
    let version = version_of(&server, &first);
    let line = || vec!["alpha size=10 Latency".to_owned()];

    let alerting = run(&server, &fixture, version, "")
        .await
        .alerting
        .expect("the first page holds the run's alerts");
    assert_eq!((alerting.total, alerting.counts.active), (2, 2));
    assert_eq!(
        alert_groups(&alerting),
        [(Some(1), first.uuid, line()), (Some(3), third.uuid, line()),]
    );
    let second = run(&server, &fixture, version, "?per_page=2&page=2").await;
    assert!(
        second.previous.is_none() && second.next.is_none() && second.alerting.is_none(),
        "only the first page carries the run's steps and alerts"
    );
    // Each history runs over its own testbed, the run's other report on it included.
    let points = alerting
        .groups
        .iter()
        .map(|group| group.points.x.len())
        .collect::<Vec<_>>();
    assert_eq!(points, [3, 4]);

    dismiss(&server, &fixture, &first).await;
    let alerting = run(&server, &fixture, version, "")
        .await
        .alerting
        .expect("the first page holds the run's alerts");
    assert_eq!(alerting.total, 2);
    assert_eq!((alerting.counts.active, alerting.counts.dismissed), (1, 1));
    let dismissed = alerting.groups[0].alerts[0]
        .line
        .alert
        .expect("the line follows its alert");
    assert_eq!(dismissed.status, AlertStatus::Dismissed);

    archive_testbed(&server, &fixture, "elsewhere").await;
    let alerting = run(&server, &fixture, version, "")
        .await
        .alerting
        .expect("the first page holds the run's alerts");
    assert_eq!(alerting.total, 1);
    assert_eq!(alert_groups(&alerting), [(Some(1), first.uuid, line())]);
}

// Kills: a run's alerts drawn past the alerts list's page, and a total that
// counts only the alerts drawn.
#[tokio::test]
async fn console_run_alerts_hold_at_most_a_page() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "manyalerts").await;
    for (time, hash) in [("2024-02-01T00:00:00Z", H1), ("2024-02-05T00:00:00Z", H2)] {
        post(
            &server,
            &fixture,
            Post {
                iterations: vec![many(STEADY)],
                ..Post::new(time, Some(hash))
            },
        )
        .await;
    }
    let jumped = post(
        &server,
        &fixture,
        Post {
            iterations: vec![many(JUMP)],
            ..Post::new("2024-02-20T00:00:00Z", Some(SPLIT))
        },
    )
    .await;

    let alerting = run(&server, &fixture, version_of(&server, &jumped), "")
        .await
        .alerting
        .expect("the first page holds the run's alerts");
    assert_eq!(alerting.total, u32::from(MAX_CONSOLE_ALERTS_PER_PAGE) + 1);
    let drawn = alerting
        .groups
        .iter()
        .map(|group| group.alerts.len())
        .sum::<usize>();
    assert_eq!(drawn, usize::from(MAX_CONSOLE_ALERTS_PER_PAGE));
}

// Kills Previous and Next that never read past the window, that take the
// nearest run the window's read finds whatever its first start, or that keep a
// run whose report reaches the window though its first start is far before it;
// checked against the runs list's own order.
#[tokio::test]
async fn console_run_steps_read_past_the_window() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "farsteps").await;
    for (time, hash) in [
        ("2023-11-01T00:00:00Z", STEP[0]),
        ("2023-11-10T00:00:00Z", STEP[1]),
        // The first run's second report reaches the third run's window.
        ("2024-01-10T00:00:00Z", STEP[0]),
        ("2024-01-15T00:00:00Z", STEP[2]),
        ("2024-02-25T00:00:00Z", STEP[3]),
    ] {
        post(&server, &fixture, Post::new(time, Some(hash))).await;
    }
    steps_follow_the_list(&server, &fixture, &[STEP[3], STEP[2], STEP[1], STEP[0]]).await;
}

// Kills a Next that keeps the run the window finds by a report on an archived
// testbed, though the run's first visible start is past the window and a
// nearer run starts before it.
#[tokio::test]
async fn console_run_next_skips_a_run_found_by_an_archived_report() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "archivedsteps").await;
    for (time, testbed, hash) in [
        ("2024-01-01T00:00:00Z", "localhost", STEP[0]),
        ("2024-01-20T00:00:00Z", "elsewhere", STEP[2]),
        ("2024-02-10T00:00:00Z", "localhost", STEP[1]),
        ("2024-02-20T00:00:00Z", "localhost", STEP[2]),
    ] {
        post(
            &server,
            &fixture,
            Post {
                testbed,
                ..Post::new(time, Some(hash))
            },
        )
        .await;
    }
    archive_testbed(&server, &fixture, "elsewhere").await;
    steps_follow_the_list(&server, &fixture, &[STEP[2], STEP[1], STEP[0]]).await;
}

/// The runs on `main` list newest first as `heads`, and each one's Previous and Next are the
/// runs beside it in that list.
async fn steps_follow_the_list(server: &TestServer, fixture: &Fixture, heads_in_order: &[&str]) {
    let listed = runs(server, fixture, "?branch=main").await;
    let order = heads(&listed);
    assert_eq!(order, heads_in_order, "the runs list's order");
    for (index, summary) in listed.runs.iter().enumerate() {
        let run = run(server, fixture, summary.uuid, "").await;
        assert_eq!(
            (
                run.previous.as_ref().map(link_head),
                run.next.as_ref().map(link_head)
            ),
            (
                order.get(index + 1).cloned(),
                index
                    .checked_sub(1)
                    .and_then(|newer| order.get(newer).cloned())
            ),
            "{}'s steps",
            order[index]
        );
    }
}

/// Runner jobs: a report created for one exists before its results, and its job's status
/// shows until the results are in. No API takes a job through its states without a runner,
/// so the jobs here are written as the runner protocol leaves them.
#[cfg(feature = "plus")]
mod jobs {
    use bencher_json::{
        JobUuid, SpecUuid,
        project::console::{ConsoleJobStatus, JsonConsoleJob, JsonConsoleRunJobs},
        runner::JobStatus,
    };
    use diesel::OptionalExtension as _;

    use super::{
        Fixture, JsonReport, NOW, Post, ReportUuid, TestServer, at, fixture, millis, post,
        report_page, run, runs_of, schema, version_of,
    };
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    const SPEC: &str = "standard-x86";

    struct Job {
        status: JobStatus,
        created: &'static str,
        started: Option<&'static str>,
        completed: Option<&'static str>,
    }

    impl Job {
        const fn new(status: JobStatus, created: &'static str) -> Self {
            Self {
                status,
                created,
                started: None,
                completed: None,
            }
        }

        const fn started(self, started: &'static str) -> Self {
            Self {
                started: Some(started),
                ..self
            }
        }

        const fn completed(self, completed: &'static str) -> Self {
            Self {
                completed: Some(completed),
                ..self
            }
        }
    }

    /// The spec every job here runs on, made once.
    fn spec_id(server: &TestServer) -> i32 {
        let mut conn = server.db_conn();
        if let Some(id) = schema::spec::table
            .filter(schema::spec::slug.eq(SPEC))
            .select(schema::spec::id)
            .first::<i32>(&mut conn)
            .optional()
            .expect("Failed to read the spec")
        {
            return id;
        }
        let created = at(NOW);
        diesel::insert_into(schema::spec::table)
            .values((
                schema::spec::uuid.eq(SpecUuid::new()),
                schema::spec::name.eq("Standard x86"),
                schema::spec::slug.eq(SPEC),
                schema::spec::os.eq("linux"),
                schema::spec::architecture.eq("x86_64"),
                schema::spec::cpu.eq(8),
                schema::spec::memory.eq(0x0004_0000_0000i64),
                schema::spec::disk.eq(0x0019_0000_0000i64),
                schema::spec::network.eq(false),
                schema::spec::created.eq(created),
                schema::spec::modified.eq(created),
            ))
            .execute(&mut conn)
            .expect("Failed to insert the spec");
        schema::spec::table
            .filter(schema::spec::slug.eq(SPEC))
            .select(schema::spec::id)
            .first(&mut conn)
            .expect("Failed to read the spec")
    }

    /// Give a report a runner job in the given state.
    fn give_job(server: &TestServer, report: &JsonReport, job: &Job) {
        let spec_id = spec_id(server);
        let mut conn = server.db_conn();
        let (report_id, project_id, organization_id): (i32, i32, i32) = schema::report::table
            .inner_join(schema::project::table)
            .filter(schema::report::uuid.eq(report.uuid))
            .select((
                schema::report::id,
                schema::report::project_id,
                schema::project::organization_id,
            ))
            .first(&mut conn)
            .expect("Failed to read the report");
        let created = at(job.created);
        diesel::insert_into(schema::job::table)
            .values((
                schema::job::uuid.eq(JobUuid::new()),
                schema::job::organization_id.eq(organization_id),
                schema::job::project_id.eq(project_id),
                schema::job::report_id.eq(report_id),
                schema::job::source_ip.eq("127.0.0.1"),
                schema::job::spec_id.eq(spec_id),
                schema::job::config.eq("{}"),
                schema::job::timeout.eq(3600),
                schema::job::priority.eq(0),
                schema::job::status.eq(job.status),
                schema::job::started.eq(job.started.map(at)),
                schema::job::completed.eq(job.completed.map(at)),
                schema::job::created.eq(created),
                schema::job::modified.eq(created),
            ))
            .execute(&mut conn)
            .expect("Failed to insert the job");
    }

    /// A report as a runner job's starts: no results yet.
    fn submitted(time: &'static str, hash: &'static str) -> Post {
        Post {
            iterations: Vec::new(),
            ..Post::new(time, Some(hash))
        }
    }

    fn view(
        job: Option<&JsonConsoleJob>,
    ) -> Option<(ConsoleJobStatus, i64, Option<i64>, Option<i64>)> {
        job.map(|job| {
            (
                job.status,
                job.created.into(),
                job.started.map(Into::into),
                job.completed.map(Into::into),
            )
        })
    }

    fn counts(jobs: Option<&JsonConsoleRunJobs>) -> Option<(u32, u32, u32, u32, Option<i64>)> {
        jobs.map(|jobs| {
            (
                jobs.queued,
                jobs.running,
                jobs.failed,
                jobs.canceled,
                jobs.since.map(Into::into),
            )
        })
    }

    const RUN: &str = "7000000000000000000000000000000000000007";

    // Kills: a job status read as another (pending or claimed not Queued; running, completed,
    // or a runner gone quiet not Running; failed; canceled), a processed job still shown, a
    // report posted by its own job given a status, a count per status taken from the wrong
    // reports, a run's start of waiting that counts a finished job, takes a started job's
    // submission, or skips one not yet started, a job's times swapped, and a job matched to
    // the report whose id is its own.
    #[tokio::test]
    async fn console_run_shows_each_runner_job_status() {
        let server = TestServer::new_at(at(NOW)).await;
        // Another project's report comes first, so no job's id is its report's.
        let other = fixture(&server, "jobstatus-other").await;
        post(
            &server,
            &other,
            Post::new("2024-02-20T06:00:00Z", Some(RUN)),
        )
        .await;
        let fixture = fixture(&server, "jobstatus").await;
        let jobs = [
            Job::new(JobStatus::Pending, "2024-02-20T10:03:00Z"),
            Job::new(JobStatus::Claimed, "2024-02-20T10:04:00Z"),
            Job::new(JobStatus::Running, "2024-02-20T10:00:00Z").started("2024-02-20T10:05:00Z"),
            Job::new(JobStatus::Completed, "2024-02-20T10:00:00Z")
                .started("2024-02-20T10:06:00Z")
                .completed("2024-02-20T10:07:00Z"),
            Job::new(JobStatus::Unknown, "2024-02-20T10:00:00Z").started("2024-02-20T10:08:00Z"),
            Job::new(JobStatus::Failed, "2024-02-20T09:00:00Z")
                .started("2024-02-20T09:01:00Z")
                .completed("2024-02-20T09:02:00Z"),
            Job::new(JobStatus::Canceled, "2024-02-20T09:00:00Z").completed("2024-02-20T09:30:00Z"),
            Job::new(JobStatus::Processed, "2024-02-20T08:00:00Z")
                .started("2024-02-20T08:01:00Z")
                .completed("2024-02-20T08:02:00Z"),
        ];
        let mut reports = Vec::new();
        for (index, job) in jobs.iter().enumerate() {
            let time = [
                "2024-02-20T10:03:00Z",
                "2024-02-20T10:04:00Z",
                "2024-02-20T10:00:00Z",
                "2024-02-20T10:00:01Z",
                "2024-02-20T10:00:02Z",
                "2024-02-20T09:00:00Z",
                "2024-02-20T09:00:01Z",
                "2024-02-20T08:01:00Z",
            ][index];
            let report = post(&server, &fixture, submitted(time, RUN)).await;
            give_job(&server, &report, job);
            reports.push(report);
        }
        // Posted by its own CI job.
        let own = post(
            &server,
            &fixture,
            Post::new("2024-02-20T07:00:00Z", Some(RUN)),
        )
        .await;

        let listed = runs_of(&server, &fixture).await;
        assert_eq!(listed.len(), 1, "one run");
        let summary = &listed[0];
        assert_eq!(
            counts(summary.jobs.as_ref()),
            Some((2, 3, 1, 1, Some(millis("2024-02-20T10:03:00Z")))),
            "the run's jobs by status, waiting since the pending job's submission"
        );

        let page = run(&server, &fixture, version_of(&server, &own), "").await;
        assert_eq!(
            counts(page.jobs.as_ref()),
            counts(summary.jobs.as_ref()),
            "the run page's jobs"
        );
        let by_report = |uuid: ReportUuid| {
            view(
                page.reports
                    .iter()
                    .find(|report| report.uuid == uuid)
                    .expect("Failed to find the report")
                    .job
                    .as_ref(),
            )
        };
        let expected = [
            Some((
                ConsoleJobStatus::Queued,
                millis("2024-02-20T10:03:00Z"),
                None,
                None,
            )),
            Some((
                ConsoleJobStatus::Queued,
                millis("2024-02-20T10:04:00Z"),
                None,
                None,
            )),
            Some((
                ConsoleJobStatus::Running,
                millis("2024-02-20T10:00:00Z"),
                Some(millis("2024-02-20T10:05:00Z")),
                None,
            )),
            Some((
                ConsoleJobStatus::Running,
                millis("2024-02-20T10:00:00Z"),
                Some(millis("2024-02-20T10:06:00Z")),
                Some(millis("2024-02-20T10:07:00Z")),
            )),
            Some((
                ConsoleJobStatus::Running,
                millis("2024-02-20T10:00:00Z"),
                Some(millis("2024-02-20T10:08:00Z")),
                None,
            )),
            Some((
                ConsoleJobStatus::Failed,
                millis("2024-02-20T09:00:00Z"),
                Some(millis("2024-02-20T09:01:00Z")),
                Some(millis("2024-02-20T09:02:00Z")),
            )),
            Some((
                ConsoleJobStatus::Canceled,
                millis("2024-02-20T09:00:00Z"),
                None,
                Some(millis("2024-02-20T09:30:00Z")),
            )),
            None,
        ];
        for (report, expected) in reports.iter().zip(expected) {
            assert_eq!(
                by_report(report.uuid),
                expected,
                "report at {}",
                report.start_time
            );
        }
        assert_eq!(by_report(own.uuid), None, "a report its own job posted");
    }

    // Kills: a run whose jobs all ended still reading as waiting, a finished run given jobs,
    // and a run's end that ignores a job that ended without writing its report's end.
    #[tokio::test]
    async fn console_runs_mix_finished_and_ended_jobs() {
        let server = TestServer::new_at(at(NOW)).await;
        let fixture = fixture(&server, "jobmix").await;
        let failed = post(&server, &fixture, submitted("2024-02-20T09:00:00Z", RUN)).await;
        give_job(
            &server,
            &failed,
            &Job::new(JobStatus::Failed, "2024-02-20T09:00:00Z")
                .started("2024-02-20T09:01:00Z")
                .completed("2024-02-20T09:42:00Z"),
        );
        let processed = post(
            &server,
            &fixture,
            Post::new("2024-02-20T09:05:00Z", Some(RUN)),
        )
        .await;
        give_job(
            &server,
            &processed,
            &Job::new(JobStatus::Processed, "2024-02-20T09:00:00Z")
                .started("2024-02-20T09:05:00Z")
                .completed("2024-02-20T09:06:00Z"),
        );
        let own = "8000000000000000000000000000000000000008";
        post(
            &server,
            &fixture,
            Post::new("2024-02-21T09:00:00Z", Some(own)),
        )
        .await;

        let listed = runs_of(&server, &fixture).await;
        let ended = listed
            .iter()
            .find(|run| run.hash.as_ref().map(ToString::to_string).as_deref() == Some(RUN))
            .expect("Failed to find the run with jobs");
        assert_eq!(
            counts(ended.jobs.as_ref()),
            Some((0, 0, 1, 0, None)),
            "a run whose jobs ended waits for nothing"
        );
        assert_eq!(
            i64::from(ended.end_time),
            millis("2024-02-20T09:42:00Z"),
            "the run ends when its failed job did"
        );
        let finished = listed
            .iter()
            .find(|run| run.hash.as_ref().map(ToString::to_string).as_deref() == Some(own))
            .expect("Failed to find the finished run");
        assert!(finished.jobs.is_none(), "a run with no jobs carries none");
        assert_eq!(
            i64::from(finished.end_time),
            millis("2024-02-21T09:01:00Z"),
            "a finished run ends with its report"
        );
    }

    // Kills: a report page that leaves out its job's status or times, a processed job still
    // shown, a spec missing from a runner report, and a spec read from the testbed for a report
    // its own job posted.
    #[tokio::test]
    async fn console_report_shows_its_job_and_spec() {
        let server = TestServer::new_at(at(NOW)).await;
        let fixture: Fixture = fixture(&server, "jobreport").await;
        let running = post(&server, &fixture, submitted("2024-02-20T10:00:00Z", RUN)).await;
        give_job(
            &server,
            &running,
            &Job::new(JobStatus::Running, "2024-02-20T10:00:00Z").started("2024-02-20T10:02:00Z"),
        );
        let processed = post(
            &server,
            &fixture,
            Post::new("2024-02-20T10:05:00Z", Some(RUN)),
        )
        .await;
        give_job(
            &server,
            &processed,
            &Job::new(JobStatus::Processed, "2024-02-20T10:00:00Z")
                .started("2024-02-20T10:05:00Z")
                .completed("2024-02-20T10:06:00Z"),
        );
        // Posted by its own job on a testbed whose runs take the spec.
        let own = post(
            &server,
            &fixture,
            Post::new("2024-02-20T11:00:00Z", Some(RUN)),
        )
        .await;
        let spec_id = spec_id(&server);
        diesel::update(schema::report::table.filter(schema::report::uuid.eq(own.uuid)))
            .set(schema::report::spec_id.eq(spec_id))
            .execute(&mut server.db_conn())
            .expect("Failed to set the report's spec");

        let page = report_page(&server, &fixture, &running).await;
        assert_eq!(
            view(page.job.as_ref()),
            Some((
                ConsoleJobStatus::Running,
                millis("2024-02-20T10:00:00Z"),
                Some(millis("2024-02-20T10:02:00Z")),
                None,
            )),
            "a running job's report"
        );
        let spec = page.spec.expect("a runner report's spec");
        assert_eq!(
            (
                spec.name.as_ref(),
                spec.slug.as_ref(),
                spec.os.to_string(),
                spec.architecture.to_string(),
                u32::from(spec.cpu),
                u64::from(spec.memory),
                u64::from(spec.disk),
            ),
            (
                "Standard x86",
                SPEC,
                "linux".to_owned(),
                "x86_64".to_owned(),
                8,
                0x0004_0000_0000,
                0x0019_0000_0000,
            ),
            "the spec it ran on"
        );

        let page = report_page(&server, &fixture, &processed).await;
        assert!(page.job.is_none(), "a processed job's report is finished");
        assert!(
            page.spec.is_some(),
            "a processed job's report keeps its spec"
        );

        let page = report_page(&server, &fixture, &own).await;
        assert!(
            page.job.is_none() && page.spec.is_none(),
            "a report its own job posted has no job and no spec"
        );
    }
}

// Kills: a run's report or a report page that leaves out the version the report was read at, the
// leaf adapter its results resolved to, or how they parsed.
#[tokio::test]
async fn run_and_report_page_carry_the_parse() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "parse").await;
    let report = post(&server, &fixture, Post::new("2024-02-01T00:00:00Z", None)).await;
    let expected = (
        Some(BmfVersion::V1),
        Some(Adapter::JsonV1),
        Some(ParseOutcome::Parsed),
    );

    let run = run(&server, &fixture, version_of(&server, &report), "").await;
    let row = &run.reports[0];
    assert_eq!(
        (row.bmf_version, row.resolved_adapter, row.parse_outcome),
        expected
    );
    let page = report_page(&server, &fixture, &report).await;
    assert_eq!(
        (page.bmf_version, page.resolved_adapter, page.parse_outcome),
        expected
    );
}
