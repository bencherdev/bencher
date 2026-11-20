#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the GitHub Actions dispatch that a detached run of an older `bencher run`
//! sends as its callback.

use bencher_api_tests::{
    LogCapture, TestOrg, TestProject, TestServer, TestUser, github::GitHubServer,
};
use bencher_json::{
    Clock, JobStatus, JobUuid, JsonReport, PlanLevel, project::GitHubLinkStatus,
    runner::JobCallbackState,
};
use bencher_schema::{
    model::runner::{JobId, QueryJobCallback},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

const INSTALLATION: u64 = 2048;
const REPOSITORY: u64 = 4096;
const FULL_NAME: &str = "octo-cat/repo";
const STORED_TOKEN: &str = "github_pat_stored-token-marker";
const STORED_URL: &str = "https://api.github.com/repos/someone/elsewhere/dispatches";
const HEADER_MARKER: &str = "callback-header-marker";
const UPGRADE: &str = "Upgrade the Bencher CLI";

struct Setup {
    server: TestServer,
    log: LogCapture,
    github: GitHubServer,
    user: TestUser,
    org: TestOrg,
    project: TestProject,
}

/// A server with the GitHub App, whose GitHub answers nothing, and a claimed project with no
/// link.
async fn setup() -> Setup {
    let github = GitHubServer::start_with_headers(Vec::new()).await;
    let log = LogCapture::default();
    let server = TestServer::new_with_github(github.base(), Clock::System, log.logger()).await;
    let (user, org, project) = seed(&server).await;
    Setup {
        server,
        log,
        github,
        user,
        org,
        project,
    }
}

// Each run names a new branch and testbed, with a threshold. Kills a refusal that comes after the
// run creates its branch, testbed, threshold, report, or job, one that only a server with the app,
// a claimed project, or a link gets, one of the dispatch with a token, and one that does not tell
// the user to upgrade.
#[tokio::test]
async fn the_dispatch_with_no_token_of_an_older_cli_is_refused_at_submit() {
    let server = TestServer::new().await;
    let (user, _org, project) = seed(&server).await;
    let slug: &str = project.slug.as_ref();
    let before = created(&server);

    let (status, body) = submit(&server, &user, slug, &tokenless_dispatch()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains(UPGRADE), "{body}");
    assert_eq!(created(&server), before, "no app");

    let setup = setup().await;
    let (server, user, project) = (&setup.server, &setup.user, &setup.project);
    let slug: &str = project.slug.as_ref();
    server.link_repository(project, user, INSTALLATION, REPOSITORY, FULL_NAME);
    let before = created(server);

    let (status, body) = submit(server, user, slug, &tokenless_dispatch()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains(UPGRADE), "{body}");
    assert_eq!(created(server), before, "a linked project");
    let (status, body) = submit(server, user, slug, &bearer_dispatch()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let before = created(server);
    let (status, body) =
        submit_unclaimed(server, "dispatch-unclaimed-project", &tokenless_dispatch()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains(UPGRADE), "{body}");
    assert_eq!(created(server), before, "an unclaimed project");
    assert_eq!(
        setup.github.received().await.len(),
        0,
        "a submit never calls GitHub"
    );
}

// The dispatch with a token is an ordinary callback. Kills a free plan that still sends it, a mint
// for it, and sending it anywhere, with any token, or with any headers but those it was stored
// with.
#[tokio::test]
async fn a_dispatch_with_its_own_token_needs_bencher_plus_and_is_sent_as_stored() {
    let setup = setup().await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let free = submit_ok(&setup, &bearer_dispatch()).await;
    assert_eq!(
        callback(&setup.server, free).state,
        JobCallbackState::Skipped
    );
    setup
        .server
        .license_org(&setup.user, &setup.org, PlanLevel::Team)
        .await;
    let plus = submit_ok(&setup, &bearer_dispatch()).await;

    fire(&setup, plus).await;

    let [request] = setup
        .server
        .callback_requests()
        .try_into()
        .ok()
        .expect("one dispatch");
    assert_eq!(request.url.as_str(), STORED_URL);
    let header = |name: &str| {
        request
            .headers
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    assert_eq!(
        header("authorization"),
        Some(format!("Bearer {STORED_TOKEN}").as_str())
    );
    assert_eq!(header("x-callback-header"), Some(HEADER_MARKER));
    let callback = callback(&setup.server, plus);
    assert_eq!(
        (
            callback.state,
            callback.attempts,
            callback.status.map(u16::from)
        ),
        (JobCallbackState::Delivered, 1, Some(200))
    );
    let Setup { github, .. } = setup;
    assert_eq!(github.received().await.len(), 0, "no mint");
}

// Kills a callback that builds its report as a server without the app would, so the project in
// it carries no link status, and a report that names the linked repository.
#[tokio::test]
async fn a_dispatch_sends_the_report_with_the_link_status() {
    let setup = setup().await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    setup
        .server
        .license_org(&setup.user, &setup.org, PlanLevel::Team)
        .await;
    let mut callback = bearer_dispatch();
    callback["body"]["client_payload"]["report"] = serde_json::json!("{{ report }}");
    let job = submit_ok(&setup, &callback).await;

    fire(&setup, job).await;

    let [request] = setup
        .server
        .callback_requests()
        .try_into()
        .ok()
        .expect("one dispatch");
    let body: serde_json::Value =
        serde_json::from_str(&request.body).expect("Invalid dispatch body");
    let report: JsonReport =
        serde_json::from_value(body["client_payload"]["report"].clone()).expect("No report");
    assert_eq!(
        report.project.github_link,
        Some(GitHubLinkStatus::Connected)
    );
    assert!(!request.body.contains(FULL_NAME), "{}", request.body);
}

// Kills a report that a run, or its replay by idempotency key, returns as a server without the
// app would build it.
#[tokio::test]
async fn a_run_and_its_replay_return_the_link_status() {
    let setup = setup().await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let slug: &str = setup.project.slug.as_ref();
    let run = serde_json::json!({
        "project": slug,
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [],
    });

    let mut reports = Vec::new();
    for _ in 0..2 {
        let response = setup
            .server
            .client
            .post(setup.server.api_url("/v0/run"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&setup.user.token),
            )
            .json(&run)
            .send()
            .await
            .expect("Request failed");
        let status = response.status();
        let body = response.text().await.expect("Failed to read the body");
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let report: JsonReport = serde_json::from_str(&body).expect("Invalid report");
        reports.push(report);
    }

    let [run, replay]: [JsonReport; 2] = reports.try_into().expect("two reports");
    assert_eq!(run.uuid, replay.uuid);
    for (name, report) in [("run", run), ("replay", replay)] {
        assert_eq!(
            report.project.github_link,
            Some(GitHubLinkStatus::Connected),
            "{name}"
        );
    }
}

/// The first user, a server admin, with a fallback spec, and a claimed project of theirs.
async fn seed(server: &TestServer) -> (TestUser, TestOrg, TestProject) {
    let user = server.signup("Dispatch User", "dispatch@example.com").await;
    let response = server
        .client
        .post(server.api_url("/v0/specs"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({
            "name": "Dispatch Spec",
            "os": "linux",
            "architecture": "x86_64",
            "cpu": 2,
            "memory": 4_294_967_296i64,
            "disk": 10_737_418_240i64,
            "fallback": true
        }))
        .send()
        .await
        .expect("Failed to create a spec");
    assert_eq!(response.status(), StatusCode::CREATED, "the spec");
    let org = server.create_org(&user, "Dispatch Org").await;
    let project = server.create_project(&user, &org, "Dispatch Project").await;
    (user, org, project)
}

/// The dispatch that `bencher run` v0.6.16 and v0.6.17 compose without `--ci-callback-token`.
fn tokenless_dispatch() -> serde_json::Value {
    serde_json::json!({
        "url": STORED_URL,
        "headers": {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
        "body": dispatch_body(),
    })
}

/// The dispatch that `bencher run` composed with `--ci-callback-token`, with one more header.
fn bearer_dispatch() -> serde_json::Value {
    serde_json::json!({
        "url": STORED_URL,
        "headers": {
            "Accept": "application/vnd.github+json",
            "Authorization": format!("Bearer {STORED_TOKEN}"),
            "X-GitHub-Api-Version": "2022-11-28",
            "X-Callback-Header": HEADER_MARKER,
        },
        "body": dispatch_body(),
    })
}

fn dispatch_body() -> serde_json::Value {
    serde_json::json!({
        "event_type": "bencher_run",
        "client_payload": {
            "bencher": { "project": "{{ project.slug }}", "job": "{{ job.uuid }}" },
        },
    })
}

/// Submit a run of a job with `callback`, and return the response status and body.
async fn submit(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
    callback: &serde_json::Value,
) -> (StatusCode, String) {
    let response = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&run(project_slug, callback))
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    (
        status,
        response.text().await.expect("Failed to read the body"),
    )
}

async fn submit_ok(setup: &Setup, callback: &serde_json::Value) -> JobUuid {
    let (status, body) = submit(
        &setup.server,
        &setup.user,
        setup.project.slug.as_ref(),
        callback,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let report: JsonReport = serde_json::from_str(&body).expect("Failed to parse the report");
    report.job.expect("The run has no job")
}

/// Submit a run without authentication, which creates `project_slug` in an unclaimed organization.
async fn submit_unclaimed(
    server: &TestServer,
    project_slug: &str,
    callback: &serde_json::Value,
) -> (StatusCode, String) {
    let mut run = run(project_slug, callback);
    if let Some(run) = run.as_object_mut() {
        run.insert(
            "context".to_owned(),
            serde_json::json!({ "bencher.dev/v0/repo/name": project_slug }),
        );
    }
    let response = server
        .client
        .post(server.api_url("/v0/run"))
        .json(&run)
        .send()
        .await
        .expect("Request failed");
    let status = response.status();
    (
        status,
        response.text().await.expect("Failed to read the body"),
    )
}

/// A run of a job with `callback`, on a new branch and testbed with a threshold.
fn run(project_slug: &str, callback: &serde_json::Value) -> serde_json::Value {
    let new = format!("dispatch-{}", uuid::Uuid::new_v4().simple());
    serde_json::json!({
        "project": project_slug,
        "branch": new,
        "testbed": new,
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [],
        "bmf_version": 1,
        "thresholds": {
            "models": [{
                "measure": "latency",
                "metric": "dispatch",
                "model": { "test": "percentage", "upper_boundary": 0.05 },
            }],
        },
        "job": {
            "image": format!("localhost/{project_slug}@sha256:{}", "a".repeat(64)),
            "callback": callback,
        },
    })
}

/// End the job, then fire its callback and wait for every delivery to settle.
async fn fire(setup: &Setup, job: JobUuid) {
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job)))
        .set(schema::job::status.eq(JobStatus::Canceled))
        .execute(&mut setup.server.db_conn())
        .expect("Failed to end the job");
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), job_id(&setup.server, job));
    setup.server.drain_callbacks().await;
}

fn job_id(server: &TestServer, job: JobUuid) -> JobId {
    schema::job::table
        .filter(schema::job::uuid.eq(job))
        .select(schema::job::id)
        .first(&mut server.db_conn())
        .expect("No job")
}

fn callback(server: &TestServer, job: JobUuid) -> QueryJobCallback {
    QueryJobCallback::get(&mut server.db_conn(), job_id(server, job)).expect("No callback")
}

/// How many branches, testbeds, thresholds, reports, and jobs the server holds.
fn created(server: &TestServer) -> [i64; 5] {
    let mut conn = server.db_conn();
    [
        schema::branch::table.count().get_result(&mut conn),
        schema::testbed::table.count().get_result(&mut conn),
        schema::threshold::table.count().get_result(&mut conn),
        schema::report::table.count().get_result(&mut conn),
        schema::job::table.count().get_result(&mut conn),
    ]
    .map(|count| count.expect("Failed to count"))
}
