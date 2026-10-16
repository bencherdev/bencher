#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The console's runs: `/v0/console/projects/{project}/runs` and
//! `/v0/console/projects/{project}/runs/{version}`.
//!
//! A run is every report with one version, which one branch head made. Every
//! report here is posted through the API with a fixed time.

use bencher_api_tests::TestServer;
use bencher_json::{
    DateTime, DateTimeMillis, JsonProjectKeyCreated, JsonReport, ProjectSlug, VersionUuid,
    project::{
        console::{ConsoleAuthorKind, JsonConsoleRun, JsonConsoleRunSummary, JsonConsoleRuns},
        report::Adapter,
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
