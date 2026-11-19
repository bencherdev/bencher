#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Each commit's title: fetched from GitHub after a report, on a new link, and on a reconnect,
//! and given beside its hash in the console's responses.

use std::{sync::Arc, time::Duration};

use bencher_api_tests::{
    LogCapture, TestServer, TestUser,
    github::{Answer, GitHubServer, HOLD, github_signature},
};
use bencher_json::{
    Clock, DateTime, DateTimeMillis, GitHubInstallationId, GitHubRepositoryId, JsonReport,
    ProjectSlug, VersionUuid,
    project::{
        console::{
            JsonConsoleAlertDetail, JsonConsoleAlerts, JsonConsolePerf, JsonConsolePointReport,
            JsonConsoleReport, JsonConsoleRun,
        },
        github::LinkedRepositoryReason,
    },
};
use bencher_schema::{model::project::linked_repository::QueryLinkedRepository, schema};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};
use http::StatusCode;
use reqwest::Method;

/// The server's frozen clock.
const NOW: i64 = 1_789_344_000;
const DAY: i64 = 24 * 60 * 60;
const INSTALLATION: u64 = 2048;
const REPOSITORY: u64 = 4096;
const TOKEN: &str = "read-token-marker";
const MINT: &str = "POST /github/app/installations/2048/access_tokens HTTP/1.1";
const GRAPHQL: &str = "POST /github/graphql HTTP/1.1";
const SKIPPED: &str =
    "Commit titles skipped: a GitHub Enterprise Server needs a Bencher Plus Enterprise license";

fn at(timestamp: i64) -> DateTime {
    DateTime::try_from(timestamp).expect("Invalid timestamp")
}

fn hash(n: u64) -> String {
    format!("{n:0>40x}")
}

struct Setup {
    server: TestServer,
    github: GitHubServer,
    user: TestUser,
    project: bencher_api_tests::TestProject,
}

/// A server with the GitHub App, whose GitHub answers with `answers` in turn.
async fn setup(answers: Vec<Answer>) -> Setup {
    let github = GitHubServer::start_with_headers(answers).await;
    let clock = Clock::Custom(Arc::new(|| at(NOW)));
    let server =
        TestServer::new_with_github(github.base(), clock, LogCapture::default().logger()).await;
    let user = server.signup("Titles User", "titles@example.com").await;
    let org = server.create_org(&user, "Titles Org").await;
    let project = server.create_project(&user, &org, "Titles Project").await;
    Setup {
        server,
        github,
        user,
        project,
    }
}

/// A server whose GitHub App is on a GitHub Enterprise Server at the fake's root, with an
/// Enterprise license on the project's organization if `licensed`.
async fn setup_enterprise(answers: Vec<Answer>, licensed: bool, log: &LogCapture) -> Setup {
    let github = GitHubServer::start_with_headers(answers).await;
    let clock = Clock::Custom(Arc::new(|| at(NOW)));
    let server = TestServer::new_with_github_enterprise(github.base(), clock, log.logger()).await;
    let user = server.signup("Titles User", "titles@example.com").await;
    let org = server.create_org(&user, "Titles Org").await;
    if licensed {
        server
            .license_org(&user, &org, bencher_json::PlanLevel::Enterprise)
            .await;
    }
    let project = server.create_project(&user, &org, "Titles Project").await;
    Setup {
        server,
        github,
        user,
        project,
    }
}

fn minted() -> Answer {
    (
        201,
        Vec::new(),
        format!(
            r#"{{"token": "{TOKEN}", "expires_at": "2026-09-12T00:00:00Z", "permissions": {{"contents": "read", "metadata": "read"}}, "repository_selection": "selected", "repositories": [{{"id": {REPOSITORY}, "name": "repo", "full_name": "octo-cat/repo", "owner": {{"login": "octo-cat", "id": 1}}}}]}}"#
        ),
    )
}

fn commit(message: &str) -> Answer {
    (
        200,
        Vec::new(),
        format!(
            r#"{{"data": {{"repository": {{"c0": {{"message": {}}}}}}}}}"#,
            serde_json::to_string(message).expect("Failed to quote the message")
        ),
    )
}

/// A report on `main` at `start`, with one latency that a threshold checks.
async fn post(
    server: &TestServer,
    slug: &ProjectSlug,
    token: &str,
    hash: &str,
    start: i64,
    latency: f64,
) -> JsonReport {
    let body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "hash": hash,
        "start_time": at(start),
        "end_time": at(start + 60),
        "results": [serde_json::json!({
            "bench": [{
                "parameters": { "size": 1 },
                "measures": { "latency": { "value": latency } },
            }],
        })
        .to_string()],
        "bmf_version": 1,
        "thresholds": {
            "models": [{
                "measure": "latency",
                "metric": "value",
                "model": { "test": "percentage", "upper_boundary": 0.25 },
            }],
        },
        "settings": { "adapter": "json" },
    });
    let response = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    let text = response.text().await.expect("Failed to read the body");
    assert_eq!(status, StatusCode::CREATED, "{text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

async fn get(server: &TestServer, token: &str, path: &str, query: &[(&str, String)]) -> String {
    let response = server
        .client
        .get(server.api_url(path))
        .query(query)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    let text = response.text().await.expect("Failed to read the body");
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    text
}

/// The title stored for the hash once the task stores its row, waiting up to ten seconds.
async fn stored(server: &TestServer, hash: &str) -> Option<String> {
    let row = || {
        schema::commit::table
            .filter(schema::commit::hash.eq(hash))
            .select(schema::commit::title)
            .first::<Option<String>>(&mut server.db_conn())
    };
    for _ in 0..200 {
        if let Ok(title) = row() {
            return title;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    row().expect("No commit row for the hash")
}

fn rows(server: &TestServer) -> i64 {
    schema::commit::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count the commits")
}

fn project_id(server: &TestServer, slug: &ProjectSlug) -> i32 {
    schema::project::table
        .filter(schema::project::slug.eq(slug))
        .select(schema::project::id)
        .first(&mut server.db_conn())
        .expect("No project")
}

/// Write a row for the project's hash, as the task would.
fn insert_commit(server: &TestServer, slug: &ProjectSlug, hash: &str, title: Option<&str>) {
    diesel::insert_into(schema::commit::table)
        .values((
            schema::commit::project_id.eq(project_id(server, slug)),
            schema::commit::hash.eq(hash),
            schema::commit::title.eq(title),
            schema::commit::created.eq(at(NOW)),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to write a commit");
}

fn request_lines(github: &[bencher_api_tests::github::Received]) -> Vec<&str> {
    github
        .iter()
        .map(bencher_api_tests::github::Received::request_line)
        .collect()
}

// Kills a report that waits on GitHub for its title, which would hold its response until the held
// answer goes out, and an ingest that never gives the task the hash.
#[tokio::test]
async fn a_report_with_a_new_hash_stores_its_title_without_waiting() {
    let (status, mut headers, body) = commit("Speed up the parser\n\nThe body.");
    headers.push((HOLD, String::new()));
    let Setup {
        server,
        github,
        user,
        project,
    } = setup(vec![minted(), (status, headers, body)]).await;
    server.link_repository(&project, &user, INSTALLATION, REPOSITORY, "octo-cat/repo");

    post(
        &server,
        &project.slug,
        &user.token,
        &hash(1),
        NOW - DAY,
        1.0,
    )
    .await;
    github.until_held().await;
    assert_eq!(rows(&server), 0);
    github.release();

    assert_eq!(
        stored(&server, &hash(1)).await.as_deref(),
        Some("Speed up the parser")
    );
    assert_eq!(request_lines(&github.received().await), [MINT, GRAPHQL]);
    server.close().await;
}

// Kills a mint or a GraphQL query on GitHub Enterprise Server that goes to github.com's layout,
// GraphQL sent under the REST API, and a licensed server that fetches no title.
#[tokio::test]
async fn a_title_on_github_enterprise_server_comes_from_its_graphql() {
    let log = LogCapture::default();
    let Setup {
        server,
        github,
        user,
        project,
    } = setup_enterprise(vec![minted(), commit("Speed up the parser")], true, &log).await;
    server.link_repository(&project, &user, INSTALLATION, REPOSITORY, "octo-cat/repo");

    post(
        &server,
        &project.slug,
        &user.token,
        &hash(1),
        NOW - DAY,
        1.0,
    )
    .await;

    assert_eq!(
        stored(&server, &hash(1)).await.as_deref(),
        Some("Speed up the parser")
    );
    assert_eq!(
        request_lines(&github.received().await),
        [
            "POST /github/api/v3/app/installations/2048/access_tokens HTTP/1.1",
            "POST /github/api/graphql HTTP/1.1",
        ]
    );
    server.close().await;
}

// The fake GitHub answers nothing, so a fetch would log a failed mint. Kills a sweep or a fetch
// on GitHub Enterprise Server without an Enterprise license, and one that skips without saying why.
#[tokio::test]
async fn without_an_enterprise_license_github_enterprise_server_gets_no_title() {
    let log = LogCapture::default();
    let Setup {
        server,
        github: _github,
        user,
        project,
    } = setup_enterprise(Vec::new(), false, &log).await;
    server.link_repository(&project, &user, INSTALLATION, REPOSITORY, "octo-cat/repo");

    post(
        &server,
        &project.slug,
        &user.token,
        &hash(1),
        NOW - DAY,
        1.0,
    )
    .await;

    let skipped = || {
        log.lines()
            .iter()
            .filter(|line| line.starts_with(SKIPPED))
            .count()
    };
    for _ in 0..200 {
        if skipped() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(rows(&server), 0);
    server.close().await;
    // One for the sweep at start, one for the report's hash.
    assert_eq!(skipped(), 2, "{:?}", log.lines());
    let lines = log.lines();
    assert!(
        !lines.iter().any(|line| line.contains("Failed to mint")),
        "{lines:?}"
    );
}

// Kills a link, a repository added back, or an unsuspended installation that leaves its project's
// titles for the next sweep, and a report on an unlinked or a disconnected project that asks
// GitHub at all.
#[tokio::test]
async fn a_new_link_and_a_reconnect_fill_their_project_at_once() {
    let Setup {
        server,
        github,
        user,
        project,
    } = setup(vec![
        minted(),
        commit("Linked title"),
        minted(),
        commit("Reconnected title"),
        minted(),
        commit("Unsuspended title"),
    ])
    .await;
    let slug = &project.slug;
    post(&server, slug, &user.token, &hash(1), NOW - DAY, 1.0).await;
    assert_eq!(rows(&server), 0);

    let state = server
        .token_key()
        .new_github_state(at(NOW), 10 * 60, user.uuid, project.uuid)
        .expect("Failed to create a state");
    let state = server
        .token_key()
        .validate_github_state(&state, at(NOW))
        .expect("Invalid state");
    let grant = server
        .token_key()
        .new_github_grant(
            at(NOW),
            10 * 60,
            &state,
            GitHubInstallationId::try_from(INSTALLATION).expect("Invalid installation"),
            GitHubRepositoryId::try_from(REPOSITORY).expect("Invalid repository"),
            "octo-cat/repo".parse().expect("Invalid name"),
        )
        .expect("Failed to create a grant");
    let response = server
        .client
        .request(
            Method::PUT,
            server.api_url(&format!("/v0/projects/{slug}/github")),
        )
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "grant": grant }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        stored(&server, &hash(1)).await.as_deref(),
        Some("Linked title")
    );

    let link = schema::linked_repository::table
        .select(QueryLinkedRepository::as_select())
        .first(&mut server.db_conn())
        .expect("No link");
    link.disconnect(&mut server.db_conn(), LinkedRepositoryReason::Removed)
        .expect("Failed to disconnect");
    post(&server, slug, &user.token, &hash(2), NOW - DAY, 1.0).await;
    assert_eq!(rows(&server), 1);
    let event = format!(
        r#"{{"action": "added", "installation": {{"id": {INSTALLATION}}}, "repository_selection": "selected", "repositories_added": [{{"id": {REPOSITORY}, "name": "repo", "full_name": "octo-cat/repo"}}], "repositories_removed": [], "requester": null, "sender": {{"login": "octo-cat"}}}}"#
    );
    let delivered = server
        .github_webhook(
            "installation_repositories",
            Some(&github_signature(&event)),
            &event,
        )
        .await;
    assert_eq!(delivered.status(), StatusCode::ACCEPTED);
    assert_eq!(
        stored(&server, &hash(2)).await.as_deref(),
        Some("Reconnected title")
    );

    for action in ["suspend", "unsuspend"] {
        if action == "unsuspend" {
            post(&server, slug, &user.token, &hash(3), NOW - DAY, 1.0).await;
            assert_eq!(rows(&server), 2);
        }
        let event = format!(
            r#"{{"action": "{action}", "installation": {{"id": {INSTALLATION}}}, "sender": {{"login": "octo-cat"}}}}"#
        );
        let delivered = server
            .github_webhook("installation", Some(&github_signature(&event)), &event)
            .await;
        assert_eq!(delivered.status(), StatusCode::ACCEPTED, "{action}");
    }
    assert_eq!(
        stored(&server, &hash(3)).await.as_deref(),
        Some("Unsuspended title")
    );

    assert_eq!(
        request_lines(&github.received().await),
        [MINT, GRAPHQL, MINT, GRAPHQL, MINT, GRAPHQL]
    );
    server.close().await;
}

fn version_of(server: &TestServer, report: &JsonReport) -> VersionUuid {
    schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(schema::report::uuid.eq(report.uuid))
        .select(schema::version::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to read the report's version")
}

/// Each point report's hash and title.
fn titles(reports: &[JsonConsolePointReport]) -> Vec<(String, Option<String>)> {
    reports
        .iter()
        .map(|report| {
            (
                report
                    .hash
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                report.title.as_ref().map(ToString::to_string),
            )
        })
        .collect()
}

// Kills a console response that leaves out the title, gives one for a row with none or for
// another project's row, or misses a hash sent in upper case, and a public report that gives one.
#[tokio::test]
async fn each_console_response_carries_the_title_and_the_public_json_does_not() {
    let server = TestServer::new_at(at(NOW)).await;
    let user = server.signup("Titles User", "titles@example.com").await;
    let org = server.create_org(&user, "Titles Org").await;
    let project = server.create_project(&user, &org, "Titles Project").await;
    let other = server.create_project(&user, &org, "Other Project").await;
    let slug = &project.slug;
    let upper = hash(0xabc_def).to_uppercase();
    let first = post(&server, slug, &user.token, &hash(1), NOW - 3 * DAY, 1.0).await;
    post(&server, slug, &user.token, &hash(2), NOW - 2 * DAY, 1.0).await;
    let last = post(&server, slug, &user.token, &upper, NOW - DAY, 10.0).await;
    insert_commit(&server, slug, &hash(1), Some("First title"));
    insert_commit(&server, slug, &hash(2), None);
    insert_commit(&server, slug, &hash(0xabc_def), Some("Third title"));
    insert_commit(&server, &other.slug, &hash(2), Some("Another title"));
    let expected = vec![
        (hash(1), Some("First title".to_owned())),
        (hash(2), None),
        (upper.clone(), Some("Third title".to_owned())),
    ];

    let report: JsonConsoleReport = serde_json::from_str(
        &get(
            &server,
            &user.token,
            &format!("/v0/console/projects/{slug}/reports/{}", last.uuid),
            &[],
        )
        .await,
    )
    .expect("Failed to parse the report");
    assert_eq!(
        report.title.as_ref().map(ToString::to_string).as_deref(),
        Some("Third title")
    );
    assert_eq!(titles(&report.reports), expected);

    let run: JsonConsoleRun = serde_json::from_str(
        &get(
            &server,
            &user.token,
            &format!(
                "/v0/console/projects/{slug}/runs/{}",
                version_of(&server, &first)
            ),
            &[],
        )
        .await,
    )
    .expect("Failed to parse the run");
    assert_eq!(
        run.title.as_ref().map(ToString::to_string).as_deref(),
        Some("First title")
    );

    let result = &last.results.as_ref().expect("No results")[0][0];
    let perf: JsonConsolePerf = serde_json::from_str(
        &get(
            &server,
            &user.token,
            &format!("/v0/console/projects/{slug}/perf"),
            &[
                ("branches", last.branch.uuid.to_string()),
                ("testbeds", last.testbed.uuid.to_string()),
                ("benchmarks", result.benchmark.uuid.to_string()),
                ("measures", result.measures[0].measure.uuid.to_string()),
                (
                    "start_time",
                    i64::from(DateTimeMillis::from(at(NOW - 4 * DAY))).to_string(),
                ),
            ],
        )
        .await,
    )
    .expect("Failed to parse the perf");
    assert_eq!(titles(&perf.reports), expected);

    let alerts: JsonConsoleAlerts = serde_json::from_str(
        &get(
            &server,
            &user.token,
            &format!("/v0/console/projects/{slug}/alerts"),
            &[],
        )
        .await,
    )
    .expect("Failed to parse the alerts");
    assert_eq!(alerts.groups.len(), 1);
    assert_eq!(titles(&alerts.reports), expected);

    let alert = last
        .alerts
        .as_ref()
        .and_then(|alerts| alerts.first())
        .expect("No alert")
        .uuid;
    let detail: JsonConsoleAlertDetail = serde_json::from_str(
        &get(
            &server,
            &user.token,
            &format!("/v0/console/projects/{slug}/alerts/{alert}"),
            &[],
        )
        .await,
    )
    .expect("Failed to parse the alert");
    assert_eq!(titles(&detail.perf.reports), expected);

    for path in [
        format!("/v0/projects/{slug}/reports/{}", last.uuid),
        format!("/v0/projects/{slug}/reports"),
    ] {
        let public = get(&server, &user.token, &path, &[]).await;
        assert!(!public.contains("Third title"), "{path}: {public}");
        assert!(!public.contains("\"title\""), "{path}: {public}");
    }
    server.close().await;
}
