#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    reason = "integration test file"
)]
//! Integration tests for Bencher's post of a detached run's GitHub check and pull request comment.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bencher_api_tests::{
    LogCapture, TestOrg, TestProject, TestServer, TestUser, github::github_signature,
};
use bencher_json::{
    Clock, JobStatus, JobUuid, JsonReport, PlanLevel,
    project::github::{LinkedRepositoryReason, LinkedRepositoryStatus},
    runner::{JobGitHubFailure, JobGitHubState},
};
use bencher_schema::{
    model::{
        project::linked_repository::QueryLinkedRepository,
        runner::{JobId, QueryJobGitHub},
    },
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use http::StatusCode;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinHandle,
};

const INSTALLATION: u64 = 2048;
const MOVED_INSTALLATION: u64 = 8192;
const REPOSITORY: u64 = 4096;
const OTHER_REPOSITORY: u64 = 4097;
const FULL_NAME: &str = "octo-cat/repo";
const RENAMED: &str = "octo-cat/renamed";
const TOKEN: &str = "ghs_post-token-marker";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER_SHA: &str = "fedcba9876543210fedcba9876543210fedcba98";
const NUMBER: u64 = 7;
const BOT: &str = "bencher-test[bot]";
const APPROVAL_URL: &str = "https://github.com/organizations/octo-cat/settings/installations/2048";
const PERMISSIONS_NOT_GRANTED: &str =
    "The permissions requested are not granted to this installation.";
const REPOSITORY_INACCESSIBLE: &str = "There is at least one repository that does not exist or is not accessible to the parent installation.";
const MINT: &str = "POST /github/app/installations/2048/access_tokens";
const MINT_MOVED: &str = "POST /github/app/installations/8192/access_tokens";
const INSTALLATION_PAGE: &str = "GET /github/app/installations/2048";
const LOOKUP: &str = "GET /github/repos/octo-cat/repo/installation";
const CREATE_CHECK: &str = "POST /github/repos/octo-cat/repo/check-runs";
const UPDATE_CHECK: &str = "PATCH /github/repos/octo-cat/repo/check-runs/";
const HEAD: &str = "GET /github/repos/octo-cat/repo/pulls/7";
const COMMENTS: &str = "GET /github/repos/octo-cat/repo/issues/7/comments";
const CREATE_COMMENT: &str = "POST /github/repos/octo-cat/repo/issues/7/comments";
const UPDATE_COMMENT: &str = "PATCH /github/repos/octo-cat/repo/issues/comments/";
const ENTERPRISE_API_URL: &str = "https://github.example.com/api/v3";
/// Long enough for a retry after its wait, and well short of a fallback pause.
const SETTLE: Duration = Duration::from_secs(30);

// Kills a comment written before the check is complete, a start that creates no check in
// progress, a check with an `external_id`, a completion that is not the job's conclusion, a mint
// for any permission but checks and pull requests write or for any other repository, a request
// without the minted token, and a minted token that reaches a stored row or a log line.
#[tokio::test]
async fn a_detached_run_gets_its_check_completed_then_its_comment() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
        ]
    );
    let requests = setup.github.requests();
    for request in &requests {
        if request.line.starts_with(MINT) {
            assert_eq!(
                request.body,
                serde_json::json!({
                    "repository_ids": [REPOSITORY],
                    "permissions": { "checks": "write", "pull_requests": "write" },
                })
            );
        } else {
            assert_eq!(
                request.authorization.as_deref(),
                Some(format!("Bearer {TOKEN}").as_str()),
                "{}",
                request.line
            );
        }
    }
    let name = "Bencher Report (Post Project)";
    let started = &requests[1].body;
    assert_eq!(started["name"], name);
    assert_eq!(started["head_sha"], SHA);
    assert_eq!(started["status"], "in_progress");
    let completed = &requests[3].body;
    assert_eq!(completed["name"], name);
    assert_eq!(completed["conclusion"], "success");
    for check in [started, completed] {
        assert!(check.get("external_id").is_none(), "{check}");
        assert!(
            check["output"]["summary"]
                .as_str()
                .unwrap()
                .contains("Bencher Report"),
            "{check}"
        );
    }
    let comment = requests[6].body["body"].as_str().unwrap();
    assert!(
        comment.ends_with(&format!(
            r#"<div id="bencher.dev/projects/post-project/sha/{SHA}"></div>"#
        )),
        "{comment}"
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
    assert_eq!(setup.job_json(job).await["github"]["state"], "posted");
    setup.assert_no_token();
}

// Kills each refusal at submit, a refusal of a link to the run's repository, a refusal that
// comes after the run creates its branch, testbed, threshold, report, or job, a submit that calls
// GitHub, and a job that reports through both a callback and GitHub.
#[tokio::test]
async fn a_run_that_could_never_post_is_refused_at_submit() {
    let server = TestServer::new().await;
    let (user, project) = seed(&server).await;
    let before = created(&server);
    let (status, body) = submit(&server, &user, &project, &github(Some(NUMBER))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(created(&server), before, "no app");

    let setup = Setup::new(Fake::default()).await;
    let (server, user, project) = (&setup.server, &setup.user, &setup.project);
    diesel::delete(schema::linked_repository::table)
        .execute(&mut server.db_conn())
        .unwrap();
    let refused = async |what: &str, run_github: serde_json::Value, expected: StatusCode| {
        let before = created(server);
        let (status, body) = submit(server, user, project, &run_github).await;
        assert_eq!(status, expected, "{what}: {body}");
        assert_eq!(created(server), before, "{what}");
        body
    };

    let body = refused("no link", github(Some(NUMBER)), StatusCode::CONFLICT).await;
    assert!(body.contains("Settings"), "{body}");
    let link = setup.link(INSTALLATION, REPOSITORY);
    link.disconnect(&mut server.db_conn(), LinkedRepositoryReason::Removed)
        .unwrap();
    let body = refused("disconnected", github(Some(NUMBER)), StatusCode::CONFLICT).await;
    assert!(body.contains(FULL_NAME), "{body}");
    setup.link(INSTALLATION, REPOSITORY);
    let mut other = github(Some(NUMBER));
    other["repository_id"] = OTHER_REPOSITORY.into();
    let body = refused("another repository", other, StatusCode::CONFLICT).await;
    assert!(body.contains(FULL_NAME), "{body}");
    let before = created(server);
    let (status, body) = submit_job(
        server,
        user,
        project.slug.as_ref(),
        serde_json::json!({
            "image": image(project.slug.as_ref()),
            "github": github(Some(NUMBER)),
            "callback": { "url": "https://receiver.example/hook" },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(created(server), before, "a callback too");

    let before = created(server);
    let unclaimed = format!("post-{}", uuid::Uuid::new_v4().simple());
    let (status, body) = submit_unclaimed(server, &unclaimed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(created(server), before, "unclaimed");

    let (status, body) = submit(server, user, project, &github(Some(NUMBER))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    server.drain_callbacks().await;
    assert_eq!(
        setup
            .github
            .lines()
            .iter()
            .filter(|line| line.starts_with(MINT))
            .count(),
        1,
        "only the accepted run's start calls GitHub"
    );
}

// Kills a server that takes a run on any GitHub but its own, that reads a run with no API URL as
// on any GitHub but github.com, that compares the API URLs with a trailing slash, and a refusal
// that comes after the run creates anything, that does not name the server's GitHub, or that
// names the run's API with a slash it did not send.
#[tokio::test]
async fn a_run_on_another_github_is_refused_at_submit() {
    let setup = Setup::new(Fake::default()).await;
    let enterprise = Setup::new_enterprise(Fake::default(), true).await;
    for (setup, api_url, accepted, named) in [
        (
            &setup,
            Some(ENTERPRISE_API_URL.to_owned()),
            false,
            "https://github.com",
        ),
        (&setup, Some("https://api.github.com/".to_owned()), true, ""),
        (&setup, None, true, ""),
        (&enterprise, None, false, enterprise.github.base.as_str()),
        (
            &enterprise,
            Some("https://api.github.com".to_owned()),
            false,
            enterprise.github.base.as_str(),
        ),
        (
            &enterprise,
            Some(format!("{}/api/v3/", enterprise.github.base)),
            true,
            "",
        ),
    ] {
        let mut run_github = github(None);
        if let Some(api_url) = &api_url {
            run_github["api_url"] = api_url.as_str().into();
        }
        let before = created(&setup.server);
        let (status, body) = submit(&setup.server, &setup.user, &setup.project, &run_github).await;
        if accepted {
            assert_eq!(status, StatusCode::CREATED, "{api_url:?}: {body}");
        } else {
            assert_eq!(status, StatusCode::CONFLICT, "{api_url:?}: {body}");
            assert!(
                body.contains(named.trim_end_matches('/')),
                "{named} not in {body}"
            );
            let sent = api_url.as_deref().unwrap_or("https://api.github.com");
            assert!(body.contains(&format!("({sent})")), "{sent} not in {body}");
            assert_eq!(created(&setup.server), before, "{api_url:?}");
        }
    }
}

// A run on GitHub Enterprise Server posts there, every request under the root's REST API. Kills a
// mint, a check, or a comment sent to github.com's layout, and a licensed server that skips its
// post.
#[tokio::test]
async fn a_post_on_github_enterprise_server_goes_to_its_api() {
    let setup = Setup::new_enterprise(Fake::default(), true).await;
    let job = setup
        .submit_ok(&setup.enterprise_github(Some(NUMBER)))
        .await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    let api = |line: &str| line.replacen("/github/", "/github/api/v3/", 1);
    assert_eq!(
        setup.github.lines(),
        [
            api(MINT),
            api(CREATE_CHECK),
            api(MINT),
            api("PATCH /github/repos/octo-cat/repo/check-runs/1"),
            api(HEAD),
            api(COMMENTS),
            api(CREATE_COMMENT),
        ]
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills a submit, a start, or a post to GitHub Enterprise Server without an Enterprise license,
// a post without one that settles, counts an attempt, or logs no reason, and a skipped post that
// does not complete its check once the license is back.
#[tokio::test]
async fn github_enterprise_server_needs_an_enterprise_license() {
    let unlicensed = Setup::new_enterprise(Fake::default(), false).await;
    let before = created(&unlicensed.server);
    let (status, body) = submit(
        &unlicensed.server,
        &unlicensed.user,
        &unlicensed.project,
        &unlicensed.enterprise_github(Some(NUMBER)),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    assert!(body.contains("Enterprise license"), "{body}");
    assert_eq!(created(&unlicensed.server), before);

    let setup = Setup::new_enterprise(Fake::default(), true).await;
    let job = setup
        .submit_ok(&setup.enterprise_github(Some(NUMBER)))
        .await;
    setup.until_checks(1).await;
    let requests = setup.github.lines().len();
    let licenses = setup.unlicense();

    setup.fire(job, JobStatus::Processed);
    setup
        .until_logged(
            "GitHub post skipped: a GitHub Enterprise Server needs a Bencher Plus Enterprise license",
            setup.github.base.as_str().trim_end_matches('/'),
        )
        .await;

    assert_eq!(setup.github.lines().len(), requests);
    let post = setup.post(job);
    assert_eq!((post.state, post.attempts), (JobGitHubState::Pending, 0));

    setup.relicense(licenses);
    setup.fire_pending().await;
    setup.server.drain_callbacks().await;

    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
    assert_eq!(setup.github.created_checks(), 1);
}

// Kills a startup sweep that passes over a check never started or a pending post, and a start
// on restart that creates a second check.
#[tokio::test]
async fn a_restart_starts_a_check_never_started_and_fires_a_pending_post() {
    let mut fake = Fake::default();
    fake.fail(MINT, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "outcome=5xx")
        .await;
    assert_eq!(setup.post(job).check_run_id, None);

    setup.fire_pending().await;
    setup.until_checks(1).await;
    setup.end(job, JobStatus::Failed);
    setup.fire_pending().await;
    setup.until_state(job, JobGitHubState::Posted).await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
        ]
    );
    assert_eq!(setup.github.requests()[2].body["status"], "in_progress");
    assert_eq!(setup.github.requests()[4].body["conclusion"], "failure");
}

// Kills a start that creates a check in progress for a job that has already finished.
#[tokio::test]
async fn a_start_that_finds_its_job_finished_creates_one_completed_check() {
    let mut fake = Fake::default();
    fake.fail(MINT, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "outcome=5xx")
        .await;

    setup.end(job, JobStatus::Canceled);
    setup
        .server
        .context()
        .callbacks
        .start(&setup.log.logger(), setup.job_id(job));
    setup.until_state(job, JobGitHubState::Posted).await;

    assert_eq!(setup.github.lines(), [MINT, MINT, CREATE_CHECK]);
    let created = &setup.github.requests()[2].body;
    assert_eq!(created["conclusion"], "cancelled");
    assert!(created.get("status").is_none(), "{created}");
}

// Kills a post that runs beside its start, which would create a second check, and a post that
// does not read the check its start recorded.
#[tokio::test]
async fn a_post_that_begins_while_its_start_creates_the_check_completes_that_check() {
    let mut fake = Fake::default();
    fake.hold(CREATE_CHECK);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.github.until_held().await;

    setup.end(job, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));
    tokio::time::sleep(Duration::from_millis(256)).await;
    setup.github.release();
    setup.server.drain_callbacks().await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
        ]
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills a refusal for want of approval that the post retries, or that it fails without the
// installation's page, a failed post that a finished job posts again, and a settle that is not
// logged as the failure it is.
#[tokio::test]
async fn a_mint_refused_for_the_new_permissions_fails_the_post_with_the_approval_link() {
    let mut fake = Fake::default();
    fake.refuse_mint(INSTALLATION, 422, PERMISSIONS_NOT_GRANTED);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_state(job, JobGitHubState::Failed).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(setup.github.lines(), [MINT, INSTALLATION_PAGE]);
    let post = setup.post(job);
    assert_eq!(post.failure, Some(JobGitHubFailure::Approval));
    assert_eq!(post.status.map(u16::from), Some(422));
    let github = &setup.job_json(job).await["github"];
    assert_eq!(github["state"], "failed");
    assert_eq!(github["failure"], "approval");
    assert_eq!(github["approval_url"], APPROVAL_URL);
    setup.assert_logged("GitHub post failed", &["failure=Approval", "status=422"]);
}

// Kills a settle that finds the post no longer pending and still logs it, and tells Sentry, as a
// failure.
#[tokio::test]
async fn a_post_settled_meanwhile_is_not_logged_as_failed() {
    let mut fake = Fake::default();
    fake.refuse_mint(INSTALLATION, 422, PERMISSIONS_NOT_GRANTED);
    fake.hold(INSTALLATION_PAGE);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.github.until_held().await;
    diesel::update(schema::job_github::table)
        .set(schema::job_github::state.eq(JobGitHubState::Posted))
        .execute(&mut setup.server.db_conn())
        .unwrap();

    setup.github.release();
    setup
        .until(
            || setup.logged("GitHub post is no longer pending", &[]),
            "the settle",
        )
        .await;

    assert!(!setup.logged("GitHub post failed", &[]));
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills a refusal for a repository that left the installation that neither moves nor disconnects
// the link, a disconnect for the wrong reason, and a post that goes on without a link.
#[tokio::test]
async fn a_mint_refused_for_a_repository_that_left_moves_or_disconnects_the_link() {
    for (status, message, reason) in [
        (404, "Not Found", LinkedRepositoryReason::Uninstalled),
        (
            422,
            REPOSITORY_INACCESSIBLE,
            LinkedRepositoryReason::Removed,
        ),
    ] {
        let mut fake = Fake::default();
        fake.refuse_mint(INSTALLATION, status, message);
        let setup = Setup::new(fake).await;
        let job = setup.submit_ok(&github(None)).await;
        setup.until_state(job, JobGitHubState::Failed).await;
        assert_eq!(setup.github.lines(), [MINT, LOOKUP], "{status}");
        assert_eq!(
            setup.post(job).failure,
            Some(JobGitHubFailure::Unlinked),
            "{status}"
        );
        let link = setup.stored_link();
        assert_eq!(
            (link.status, link.reason),
            (LinkedRepositoryStatus::Disconnected, Some(reason)),
            "{status}"
        );
    }

    let mut fake = Fake::default();
    fake.refuse_mint(INSTALLATION, 422, REPOSITORY_INACCESSIBLE);
    fake.install(MOVED_INSTALLATION);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    assert_eq!(
        setup.github.lines(),
        [MINT, LOOKUP, MINT_MOVED, CREATE_CHECK]
    );
    assert_eq!(
        setup.linked(),
        (MOVED_INSTALLATION, LinkedRepositoryStatus::Connected)
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Pending);
}

// A renamed or transferred repository's lookup answers with a redirect. Kills a redirect not
// followed on the API's origin, and one followed anywhere else.
#[tokio::test]
async fn a_lookup_redirected_on_the_api_origin_moves_the_link() {
    let mut fake = Fake::default();
    fake.fail(MINT, 404);
    fake.redirect_lookup("/github/repositories/4096/installation");
    fake.install(MOVED_INSTALLATION);
    let setup = Setup::new(fake).await;
    setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            LOOKUP,
            "GET /github/repositories/4096/installation",
            MINT_MOVED,
            CREATE_CHECK,
        ]
    );
    assert_eq!(
        setup.linked(),
        (MOVED_INSTALLATION, LinkedRepositoryStatus::Connected)
    );

    let mut fake = Fake::default();
    fake.fail(MINT, 404);
    fake.redirect_lookup("https://elsewhere.example/repositories/4096/installation");
    fake.install(MOVED_INSTALLATION);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "stage=lookup")
        .await;
    setup.server.drain_callbacks().await;
    assert_eq!(setup.github.lines(), [MINT, LOOKUP]);
    assert_eq!(
        setup.linked(),
        (INSTALLATION, LinkedRepositoryStatus::Connected)
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Pending);
}

// The mint answers with the repository's name now, which a rename or a transfer changes. Kills a
// post that sends its requests to the stored name, and a new name never written back.
#[tokio::test]
async fn a_renamed_repository_gets_its_post_at_its_new_name() {
    let setup = Setup::new(Fake::default().renamed(RENAMED)).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            "POST /github/repos/octo-cat/renamed/check-runs",
            MINT,
            "PATCH /github/repos/octo-cat/renamed/check-runs/1",
            "GET /github/repos/octo-cat/renamed/pulls/7",
            "GET /github/repos/octo-cat/renamed/issues/7/comments",
            "POST /github/repos/octo-cat/renamed/issues/7/comments",
        ]
    );
    assert_eq!(setup.stored_link().full_name.to_string(), RENAMED);
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills a second refusal that leaves the moved link connected.
#[tokio::test]
async fn a_moved_link_refused_again_disconnects() {
    let mut fake = Fake::default();
    fake.refuse_mint(INSTALLATION, 404, "Not Found");
    fake.refuse_mint(MOVED_INSTALLATION, 404, "Not Found");
    fake.install(MOVED_INSTALLATION);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_state(job, JobGitHubState::Failed).await;

    assert_eq!(setup.github.lines(), [MINT, LOOKUP, MINT_MOVED]);
    assert_eq!(setup.post(job).failure, Some(JobGitHubFailure::Unlinked));
    let link = setup.stored_link();
    assert_eq!(
        (u64::from(link.installation_id), link.status, link.reason),
        (
            MOVED_INSTALLATION,
            LinkedRepositoryStatus::Disconnected,
            Some(LinkedRepositoryReason::Uninstalled)
        )
    );
}

// A refusal that leaves the link as it is asks nothing more of GitHub. Kills a lookup, and with it
// a move or a disconnect, after a 403.
#[tokio::test]
async fn a_refusal_that_keeps_the_link_looks_up_nothing() {
    let mut fake = Fake::default();
    fake.fail(MINT, 403);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "status=403")
        .await;
    setup.server.drain_callbacks().await;

    assert_eq!(setup.github.lines(), [MINT]);
    assert_eq!(
        setup.linked(),
        (INSTALLATION, LinkedRepositoryStatus::Connected)
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Pending);
}

// Kills a failed lookup that disconnects the link or fails the post, which a later attempt can
// still make.
#[tokio::test]
async fn a_failed_lookup_leaves_the_link() {
    let mut fake = Fake::default();
    fake.fail(MINT, 404);
    fake.fail(LOOKUP, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "stage=lookup")
        .await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(setup.github.lines(), [MINT, LOOKUP, MINT, CREATE_CHECK]);
    assert_eq!(
        setup.linked(),
        (INSTALLATION, LinkedRepositoryStatus::Connected)
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// A lookup holds its answer while the project's link is replaced, so the move finds no link to
// move. Kills a mint once more for the link the attempt read, which the project no longer has,
// and a move or a disconnect that lands on the link that replaced it.
#[tokio::test]
async fn a_link_replaced_during_a_lookup_is_left_alone() {
    let mut fake = Fake::default();
    fake.fail(MINT, 404);
    fake.install(MOVED_INSTALLATION);
    fake.hold(LOOKUP);
    let setup = Setup::new(fake).await;
    setup.submit_ok(&github(None)).await;
    setup.github.until_held().await;

    setup.link(INSTALLATION, OTHER_REPOSITORY);
    setup.github.release();
    setup
        .until_logged("GitHub post request", "status=404")
        .await;
    setup.server.drain_callbacks().await;

    assert_eq!(setup.github.lines(), [MINT, LOOKUP]);
    let link = setup.stored_link();
    assert_eq!(
        (
            u64::from(link.installation_id),
            u64::from(link.repository_id),
            link.status,
        ),
        (
            INSTALLATION,
            OTHER_REPOSITORY,
            LinkedRepositoryStatus::Connected
        )
    );
}

// The pacer reads a refused mint as it reads any refused request. Kills a 5xx from the mint that
// ends the post, and a 429's `retry-after` or a spent limit's reset that never reaches the pacer,
// which would then wait out a minute's fallback pause.
#[tokio::test]
async fn a_refused_mint_is_retried_when_its_answer_says_to() {
    let reset = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 1)
    .to_string();
    for (status, headers) in [
        (500, Vec::new()),
        (429, vec![("retry-after", "1")]),
        (
            403,
            vec![
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", reset.as_str()),
            ],
        ),
    ] {
        let setup = Setup::new(Fake::default()).await;
        let job = setup.submit_ok(&github(None)).await;
        setup.until_checks(1).await;
        setup.github.fail_with(MINT, status, &headers);
        let fired = Instant::now();

        setup.end_and_fire(job, JobStatus::Processed).await;

        assert!(fired.elapsed() < SETTLE, "{status}: {:?}", fired.elapsed());
        assert_eq!(
            setup.github.lines()[2..],
            [MINT, MINT, "PATCH /github/repos/octo-cat/repo/check-runs/1"],
            "{status}"
        );
        let post = setup.post(job);
        assert_eq!(
            (post.state, post.attempts, post.status.map(u16::from)),
            (JobGitHubState::Posted, 2, Some(status)),
            "{status}"
        );
    }
}

// Kills a mint that never answers classed as anything but a timeout, and one that ends the post.
#[tokio::test]
async fn a_mint_timeout_is_retried() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    setup.github.silence(MINT);

    setup.end_and_fire(job, JobStatus::Processed).await;

    setup.assert_logged("GitHub post request", &["stage=mint", "outcome=timeout"]);
    assert_eq!(
        setup.github.lines()[2..],
        [MINT, MINT, "PATCH /github/repos/octo-cat/repo/check-runs/1"]
    );
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.status),
        (JobGitHubState::Posted, 2, None)
    );
}

// Kills pacing a post by anything but its installation, which would hold every other
// installation's posts behind one installation's rate limit. The 429's `retry-after` pauses its
// own installation for longer than the test waits.
#[tokio::test]
async fn a_rate_limit_pauses_only_its_installation() {
    let setup = Setup::new(Fake::default()).await;
    let org = setup.server.create_org(&setup.user, "Other Post Org").await;
    let other = setup
        .server
        .create_project(&setup.user, &org, "Other Post Project")
        .await;
    setup.server.link_repository(
        &other,
        &setup.user,
        MOVED_INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let limited = setup.submit_ok(&github(None)).await;
    let (status, body) = submit(&setup.server, &setup.user, &other, &github(None)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let unlimited = serde_json::from_str::<JsonReport>(&body)
        .unwrap()
        .job
        .unwrap();
    setup.until_checks(2).await;
    setup.github.fail_with(MINT, 429, &[("retry-after", "60")]);

    setup.fire(limited, JobStatus::Processed);
    setup
        .until(|| setup.post(limited).attempts == 1, "the limited attempt")
        .await;
    setup.fire(unlimited, JobStatus::Processed);
    setup
        .until(
            || setup.post(unlimited).state == JobGitHubState::Posted,
            "the unlimited post",
        )
        .await;

    let limited = setup.post(limited);
    assert_eq!(
        (limited.state, limited.status.map(u16::from)),
        (JobGitHubState::Pending, Some(429))
    );
    let Setup { server, .. } = setup;
    server.close().await;
}

// Kills reading the link once for every post: a link removed after a refused attempt fails the
// post before it mints again.
#[tokio::test]
async fn the_next_attempt_finds_a_removed_link_and_mints_nothing() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    setup.github.fail_with(MINT, 500, &[]);

    setup.fire(job, JobStatus::Processed);
    setup.until_first_attempt(job).await;
    QueryLinkedRepository::delete(&mut setup.server.db_conn(), setup.stored_link().project_id)
        .unwrap();
    setup.server.drain_callbacks().await;

    assert_eq!(setup.github.lines()[2..], [MINT]);
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.failure, post.attempts),
        (JobGitHubState::Failed, Some(JobGitHubFailure::Unlinked), 2)
    );
}

// The app is uninstalled after the start creates the check. Kills a post that reads the link
// whatever its status, which would mint and post for a disconnected link.
#[tokio::test]
async fn a_link_disconnected_before_the_fire_sends_nothing() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    let uninstalled = format!(
        r#"{{"action": "deleted", "installation": {{"id": {INSTALLATION}}}, "sender": {{"login": "octo-cat"}}}}"#
    );
    let response = setup
        .server
        .github_webhook(
            "installation",
            Some(&github_signature(&uninstalled)),
            &uninstalled,
        )
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        setup.linked(),
        (INSTALLATION, LinkedRepositoryStatus::Disconnected)
    );

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.failure),
        (JobGitHubState::Failed, Some(JobGitHubFailure::Unlinked))
    );
}

// Kills a move that lands on another project's link or on a link replaced since, that matches by
// status, or that leaves the moved link disconnected.
#[tokio::test]
async fn a_move_touches_only_the_link_it_read() {
    let setup = Setup::new(Fake::default()).await;
    let Setup {
        server,
        user,
        project,
        ..
    } = &setup;
    let org = server.create_org(user, "Other Post Org").await;
    let other = server
        .create_project(user, &org, "Other Post Project")
        .await;
    let read = setup.stored_link();
    server.link_repository(&other, user, INSTALLATION, REPOSITORY, FULL_NAME);
    let mut conn = server.db_conn();
    read.disconnect(&mut conn, LinkedRepositoryReason::Removed)
        .unwrap();

    let moved = read
        .move_to(&mut conn, installation_id(MOVED_INSTALLATION))
        .unwrap()
        .expect("The link moves");

    let state = |link: &QueryLinkedRepository| {
        (
            u64::from(link.installation_id),
            u64::from(link.repository_id),
            link.status,
            link.reason,
        )
    };
    let connected = |installation, repository| {
        (
            installation,
            repository,
            LinkedRepositoryStatus::Connected,
            None,
        )
    };
    let stored = |project: &TestProject| {
        schema::linked_repository::table
            .inner_join(schema::project::table)
            .filter(schema::project::uuid.eq(project.uuid))
            .select(QueryLinkedRepository::as_select())
            .first::<QueryLinkedRepository>(&mut server.db_conn())
            .unwrap()
    };
    assert_eq!(state(&moved), connected(MOVED_INSTALLATION, REPOSITORY));
    assert_eq!(
        state(&stored(project)),
        connected(MOVED_INSTALLATION, REPOSITORY)
    );
    assert_eq!(
        state(&stored(&other)),
        connected(INSTALLATION, REPOSITORY),
        "another project's link"
    );

    let replaced = read.move_to(&mut conn, installation_id(6000)).unwrap();
    assert!(replaced.is_none(), "a link on another installation since");
    assert_eq!(
        state(&stored(project)),
        connected(MOVED_INSTALLATION, REPOSITORY)
    );

    setup.link(INSTALLATION, OTHER_REPOSITORY);
    let replaced = read.move_to(&mut conn, installation_id(6000)).unwrap();
    assert!(replaced.is_none(), "a link to another repository since");
    assert_eq!(
        state(&stored(project)),
        connected(INSTALLATION, OTHER_REPOSITORY)
    );
}

// Kills a retry that completes the check again, a failed comment that leaves the check in
// progress, and a refused comment that the post retries.
#[tokio::test]
async fn a_failed_comment_leaves_the_check_completed() {
    let mut fake = Fake::default();
    fake.fail(CREATE_COMMENT, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert_eq!(
        setup.github.lines()[2..],
        [
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
            MINT,
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
        ]
    );
    let post = setup.post(job);
    assert_eq!((post.state, post.attempts), (JobGitHubState::Posted, 2));

    let mut fake = Fake::default();
    fake.fail(CREATE_COMMENT, 403);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert!(
        setup
            .github
            .lines()
            .contains(&"PATCH /github/repos/octo-cat/repo/check-runs/1".to_owned())
    );
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.failure, post.status.map(u16::from)),
        (
            JobGitHubState::Failed,
            Some(JobGitHubFailure::Refused),
            Some(403)
        )
    );
}

// Kills reading, updating, or deleting a comment by any author but the app's bot.
#[tokio::test]
async fn a_comment_the_app_bot_did_not_write_is_never_updated() {
    let mut fake = Fake::default();
    let tag = format!(r#"<div id="bencher.dev/projects/post-project/sha/{SHA}"></div>"#);
    fake.comment("octo-cat", "User", None, &tag);
    fake.comment("github-actions[bot]", "Bot", Some("github-actions"), &tag);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    let lines = setup.github.lines();
    assert_eq!(lines.last().unwrap(), CREATE_COMMENT);
    assert!(
        !lines.iter().any(|line| line.starts_with(UPDATE_COMMENT)),
        "{lines:?}"
    );
    let comments = setup.github.comments();
    assert_eq!(comments.len(), 3);
    for comment in &comments[..2] {
        assert_eq!(comment.body, tag);
    }
}

// Kills a guard that comments on a pull request whose head has moved or on a number that names no
// pull request, and one that leaves such a check uncompleted.
#[tokio::test]
async fn a_moved_head_or_no_pull_request_gets_a_check_and_no_comment() {
    for (fake, what) in [
        (Fake::default().with_head(OTHER_SHA), "a moved head"),
        (Fake::default().without_pull_request(), "no pull request"),
    ] {
        let setup = Setup::new(fake).await;
        let job = setup.submit_ok(&github(Some(NUMBER))).await;
        setup.until_checks(1).await;
        setup.end_and_fire(job, JobStatus::Processed).await;
        assert_eq!(
            setup.github.lines()[2..],
            [MINT, "PATCH /github/repos/octo-cat/repo/check-runs/1", HEAD],
            "{what}"
        );
        assert_eq!(setup.post(job).state, JobGitHubState::Posted, "{what}");
    }
}

// Kills a run without a pull request that reads one or comments.
#[tokio::test]
async fn a_push_run_gets_a_check_and_no_comment() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1"
        ]
    );
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills comment writes for one pull request that run side by side, which create two comments, and
// sections read from anywhere but the jobs of the project on the head SHA.
#[tokio::test]
async fn two_jobs_finishing_together_share_one_comment() {
    let setup = Setup::new(Fake::default()).await;
    let first = setup.submit_ok(&github(Some(NUMBER))).await;
    let second = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(2).await;

    for job in [first, second] {
        setup.end(job, JobStatus::Processed);
    }
    for job in [first, second] {
        setup
            .server
            .context()
            .callbacks
            .fire(&setup.log.logger(), setup.job_id(job));
    }
    setup.server.drain_callbacks().await;

    let lines = setup.github.lines();
    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    assert_eq!(
        (count(CREATE_COMMENT), count(UPDATE_COMMENT)),
        (1, 1),
        "{lines:?}"
    );
    let comments = setup.github.comments();
    let [comment] = comments.as_slice() else {
        panic!("{comments:?}");
    };
    for job in [first, second] {
        let report = setup.report(job);
        assert!(
            comment.body.contains(&report),
            "{report} in {}",
            comment.body
        );
    }
}

// Kills `--ci-only-on-alert` that stops an update of an existing comment, or that lets a new one
// be written without an alert.
#[tokio::test]
async fn only_on_alert_writes_no_new_comment_but_updates_one() {
    let mut only_on_alert = github(Some(NUMBER));
    only_on_alert["ci_only_on_alert"] = true.into();

    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&only_on_alert).await;
    setup.until_checks(1).await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert_eq!(setup.github.lines().last().unwrap(), COMMENTS);
    assert_eq!(setup.github.comments().len(), 0);

    let mut fake = Fake::default();
    let old = format!(
        r#"<h2>Old</h2><div id="bencher.dev/projects/post-project/sha/{OTHER_SHA}"></div>"#
    );
    fake.comment(BOT, "Bot", Some("bencher-test"), &old);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&only_on_alert).await;
    setup.until_checks(1).await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert_eq!(
        setup.github.lines().last().unwrap(),
        &format!("{UPDATE_COMMENT}1")
    );
    let comments = setup.github.comments();
    assert_eq!(comments.len(), 1);
    assert!(
        comments[0]
            .body
            .ends_with(&format!(r#"/sha/{SHA}"></div>"#)),
        "{}",
        comments[0].body
    );
}

// Kills a retry count that never ends, ends a retry early, or retries a third failure, and a
// failed post logged without GitHub's last answer.
#[tokio::test]
async fn three_retryable_failures_exhaust_the_post() {
    let mut fake = Fake::default();
    for _ in 0..3 {
        fake.fail(CREATE_COMMENT, 500);
    }
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    let lines = setup.github.lines();
    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    assert_eq!(
        (
            count("PATCH /github/repos/octo-cat/repo/check-runs/"),
            count(CREATE_COMMENT)
        ),
        (1, 3),
        "{lines:?}"
    );
    let post = setup.post(job);
    assert_eq!(
        (
            post.state,
            post.failure,
            post.attempts,
            post.status.map(u16::from)
        ),
        (
            JobGitHubState::Failed,
            Some(JobGitHubFailure::Exhausted),
            3,
            Some(500)
        )
    );
    setup.assert_logged(
        "GitHub post failed",
        &["attempts=3", "failure=Exhausted", "status=500"],
    );
}

// Kills an unauthorized answer to a request with the minted token that fails the post for good,
// though the token can expire while the request waits its turn, a retry that completes the check
// again, and an unauthorized mint, whose credential is the app's own, that is retried.
#[tokio::test]
async fn a_token_that_expires_during_an_attempt_is_minted_again() {
    let mut fake = Fake::default();
    fake.fail(HEAD, 401);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(
        setup.github.lines()[2..],
        [
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
            HEAD,
            MINT,
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
        ]
    );
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.status.map(u16::from)),
        (JobGitHubState::Posted, 2, Some(401))
    );

    let mut fake = Fake::default();
    for _ in 0..2 {
        fake.fail(MINT, 401);
    }
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup
        .until_logged("GitHub post request", "status=401")
        .await;
    setup.end_and_fire(job, JobStatus::Processed).await;
    assert_eq!(setup.github.lines(), [MINT, MINT]);
    let post = setup.post(job);
    assert_eq!(
        (
            post.state,
            post.failure,
            post.attempts,
            post.status.map(u16::from)
        ),
        (
            JobGitHubState::Failed,
            Some(JobGitHubFailure::Refused),
            1,
            Some(401)
        )
    );
}

// Kills a `ci_id` refused for its length, one kept whole where the check's name cuts it, and a
// cut inside a character.
#[tokio::test]
async fn a_long_ci_id_is_kept_as_the_check_name_keeps_it() {
    let kept = "a".repeat(237);
    let mut long = github(None);
    // Past 1024 bytes, with a two-byte character across the 238-byte cut.
    long["ci_id"] = format!("{kept}{}", "é".repeat(512)).into();
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&long).await;
    setup.until_checks(1).await;

    assert_eq!(setup.post(job).ci_id.map(String::from), Some(kept.clone()));
    assert_eq!(
        setup.github.requests()[1].body["name"],
        format!("Bencher Report ({kept})")
    );
}

// Kills a start that takes a database error for a lost link and fails the post for good.
#[tokio::test]
async fn a_database_error_at_the_start_leaves_the_post_pending() {
    let mut fake = Fake::default();
    fake.fail(MINT, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup
        .until_logged("GitHub post request", "outcome=5xx")
        .await;
    setup.rename_table("linked_repository", "linked_repository_hidden");

    // The first start's task can still be live, and a live task takes no second start.
    setup
        .until(
            || {
                setup
                    .server
                    .context()
                    .callbacks
                    .start(&setup.log.logger(), setup.job_id(job));
                setup.logged("Failed to read a linked repository", &[])
            },
            "a failed read",
        )
        .await;
    setup.server.drain_callbacks().await;

    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.failure),
        (JobGitHubState::Pending, 0, None)
    );
    assert_eq!(setup.github.lines(), [MINT]);
}

// Kills a post that takes a database error for a lost link and fails for good, and one that does
// not count the attempt.
#[tokio::test]
async fn a_database_error_during_a_post_counts_an_attempt_and_retries() {
    let mut fake = Fake::default();
    fake.fail(UPDATE_CHECK, 500);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    setup.end(job, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));
    setup
        .until_logged("GitHub post request", "outcome=5xx")
        .await;
    setup.rename_table("linked_repository", "linked_repository_hidden");
    setup
        .until(
            || setup.logged("Failed to read a linked repository", &[]),
            "a failed read",
        )
        .await;
    assert_eq!(setup.post(job).state, JobGitHubState::Pending);
    setup.rename_table("linked_repository_hidden", "linked_repository");

    setup.until_state(job, JobGitHubState::Posted).await;
    setup.server.drain_callbacks().await;

    let update = "PATCH /github/repos/octo-cat/repo/check-runs/1";
    assert_eq!(
        setup.github.lines(),
        [MINT, CREATE_CHECK, MINT, update, MINT, update]
    );
    assert_eq!(setup.post(job).attempts, 3);
}

// Kills a post that takes a database error while it builds its report for a report that can never
// render and fails for good, and one that does not count the attempt.
#[tokio::test]
async fn a_database_error_while_the_post_builds_its_report_counts_an_attempt_and_retries() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.rename_table("testbed", "testbed_hidden");
    setup.end(job, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));

    setup.until_first_attempt(job).await;
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.failure),
        (JobGitHubState::Pending, 1, None)
    );
    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
    setup.rename_table("testbed_hidden", "testbed");

    setup.until_state(job, JobGitHubState::Posted).await;
    setup.server.drain_callbacks().await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
        ]
    );
    assert_eq!(setup.post(job).attempts, 2);
}

// Kills a post that stops with no write and no retry once the jobs of its pull request cannot be
// read, and one that writes its comment without a section whose report cannot be read.
#[tokio::test]
async fn a_database_error_while_the_post_reads_the_pull_request_s_jobs_counts_an_attempt_and_retries()
 {
    for table in ["report", "testbed"] {
        let with_ci_id = |ci_id: &str| {
            let mut github = github(Some(NUMBER));
            github["ci_id"] = ci_id.into();
            github
        };
        let mut fake = Fake::default();
        fake.hold(HEAD);
        let setup = Setup::new(fake).await;
        let other = setup.submit_ok(&with_ci_id("alpha")).await;
        let job = setup.submit_ok(&with_ci_id("beta")).await;
        setup.until_checks(2).await;
        setup.end(job, JobStatus::Processed);
        setup
            .server
            .context()
            .callbacks
            .fire(&setup.log.logger(), setup.job_id(job));
        setup.github.until_held().await;
        let hidden = format!("{table}_hidden");
        setup.rename_table(table, &hidden);
        setup.github.release();

        setup.until_first_attempt(job).await;
        let post = setup.post(job);
        assert_eq!(
            (post.state, post.attempts, post.failure),
            (JobGitHubState::Pending, 1, None),
            "{table}"
        );
        assert!(setup.github.comments().is_empty(), "{table}");
        setup.rename_table(&hidden, table);

        setup.until_state(job, JobGitHubState::Posted).await;
        setup.server.drain_callbacks().await;

        let lines = setup.github.lines();
        let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
        assert_eq!(
            (count(UPDATE_CHECK), count(HEAD), count(CREATE_COMMENT)),
            (1, 2, 1),
            "{table}: {lines:?}"
        );
        let comments = setup.github.comments();
        let [comment] = comments.as_slice() else {
            panic!("{table}: {comments:?}");
        };
        for section in [other, job] {
            let report = setup.report(section);
            assert!(
                comment.body.contains(&report),
                "{table}: {report} in {}",
                comment.body
            );
        }
        assert_eq!(setup.post(job).attempts, 2, "{table}");
    }
}

// Kills a post that stops with no write and no retry once it cannot load itself, and one that
// does not count the attempt.
#[tokio::test]
async fn a_database_error_while_the_post_loads_counts_an_attempt_and_retries() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.end(job, JobStatus::Processed);
    setup.rename_table("organization", "organization_hidden");
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));

    setup.until_first_attempt(job).await;
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.failure),
        (JobGitHubState::Pending, 1, None)
    );
    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
    setup.rename_table("organization_hidden", "organization");

    setup.until_state(job, JobGitHubState::Posted).await;
    setup.server.drain_callbacks().await;

    assert_eq!(
        setup.github.lines(),
        [
            MINT,
            CREATE_CHECK,
            MINT,
            "PATCH /github/repos/octo-cat/repo/check-runs/1",
            HEAD,
            COMMENTS,
            CREATE_COMMENT,
        ]
    );
    assert_eq!(setup.post(job).attempts, 2);
}

// Kills a post that stays pending once its third load fails, so each restart counts 3 more, a
// load limit of more than 3, and a failed post logged for a job that has no post.
#[tokio::test]
async fn three_failed_loads_exhaust_the_post() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    let slug: &str = setup.project.slug.as_ref();
    let (status, body) = submit_job(
        &setup.server,
        &setup.user,
        slug,
        serde_json::json!({ "image": image(slug) }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let unposted = serde_json::from_str::<JsonReport>(&body)
        .unwrap()
        .job
        .unwrap();
    setup.until_checks(1).await;
    setup.rename_table("organization", "organization_hidden");
    setup.end(unposted, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(unposted));

    setup.end_and_fire(job, JobStatus::Processed).await;

    let post = setup.post(job);
    assert_eq!(
        (post.state, post.failure, post.attempts, post.status),
        (
            JobGitHubState::Failed,
            Some(JobGitHubFailure::Exhausted),
            3,
            None
        )
    );
    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
    let lines = setup.log.lines();
    let failed: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("GitHub post failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    setup.assert_logged("GitHub post failed", &["failure=Exhausted"]);
}

// Kills a start that counts one of the post's attempts when it cannot load the post.
#[tokio::test]
async fn a_start_that_cannot_load_its_post_counts_no_attempt() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(None)).await;
    setup.until_checks(1).await;
    setup.rename_table("organization", "organization_hidden");

    // The first start's task can still be live, and a live task takes no second start.
    setup
        .until(
            || {
                setup
                    .server
                    .context()
                    .callbacks
                    .start(&setup.log.logger(), setup.job_id(job));
                setup.logged("Failed to load a GitHub post", &[])
            },
            "a failed load",
        )
        .await;
    setup.server.drain_callbacks().await;

    let post = setup.post(job);
    assert_eq!(
        (post.state, post.attempts, post.failure),
        (JobGitHubState::Pending, 0, None)
    );
    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
}

// Kills a shutdown that counts an attempt or settles the post it cuts short.
#[tokio::test]
async fn a_shutdown_cuts_a_post_short_without_a_write() {
    let mut fake = Fake::default();
    fake.hold(UPDATE_CHECK);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.end(job, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));
    setup.github.until_held().await;
    let before = format!("{:?}", setup.post(job));

    setup.server.context().shutdown.cancel();
    setup.server.drain_callbacks().await;

    assert_eq!(format!("{:?}", setup.post(job)), before);
    let pending = QueryJobGitHub::pending(&mut setup.server.db_conn()).unwrap();
    assert_eq!(
        pending,
        [(setup.job_id(job), JobStatus::Processed)],
        "the next start posts it"
    );
}

// Kills a post that sends again once a restart finds its attempts spent.
#[tokio::test]
async fn a_post_with_its_attempts_spent_sends_nothing() {
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    diesel::update(schema::job_github::table)
        .set(schema::job_github::attempts.eq(3))
        .execute(&mut setup.server.db_conn())
        .unwrap();

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(setup.github.lines(), [MINT, CREATE_CHECK]);
    let post = setup.post(job);
    assert_eq!(
        (post.state, post.failure),
        (JobGitHubState::Failed, Some(JobGitHubFailure::Exhausted))
    );
}

// Kills a post that goes on retrying once its job, and with it the post, is gone.
#[tokio::test]
async fn a_post_stops_once_its_job_is_gone() {
    let mut fake = Fake::default();
    for _ in 0..2 {
        fake.fail(CREATE_COMMENT, 500);
    }
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;
    setup.end(job, JobStatus::Processed);
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), setup.job_id(job));
    setup
        .until(|| setup.post(job).attempts == 1, "the first attempt")
        .await;

    diesel::delete(schema::report::table)
        .execute(&mut setup.server.db_conn())
        .unwrap();
    setup.server.drain_callbacks().await;

    let lines = setup.github.lines();
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with(CREATE_COMMENT))
            .count(),
        2,
        "{lines:?}"
    );
}

// Kills `--ci-only-thresholds` that comments without a threshold.
#[tokio::test]
async fn only_thresholds_writes_no_comment_without_a_threshold() {
    let mut only_thresholds = github(Some(NUMBER));
    only_thresholds["ci_only_thresholds"] = true.into();
    let setup = Setup::new(Fake::default()).await;
    let job = setup.submit_ok(&only_thresholds).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    assert_eq!(setup.github.lines().last().unwrap(), HEAD);
    assert_eq!(setup.post(job).state, JobGitHubState::Posted);
}

// Kills a search that reads only the first page, or that pages the wrong way.
#[tokio::test]
async fn the_app_bot_s_comment_is_found_past_the_first_page() {
    let mut fake = Fake::default();
    for _ in 0..100 {
        fake.comment("octo-cat", "User", None, "Looks good");
    }
    let tag = format!(r#"<div id="bencher.dev/projects/post-project/sha/{SHA}"></div>"#);
    fake.comment(BOT, "Bot", Some("bencher-test"), &tag);
    let setup = Setup::new(fake).await;
    let job = setup.submit_ok(&github(Some(NUMBER))).await;
    setup.until_checks(1).await;

    setup.end_and_fire(job, JobStatus::Processed).await;

    let requests = setup.github.requests();
    let pages: Vec<&str> = requests
        .iter()
        .filter(|request| request.line.starts_with(COMMENTS))
        .map(|request| request.line.as_str())
        .collect();
    assert_eq!(
        pages,
        [
            format!("{COMMENTS}?per_page=100&page=1"),
            format!("{COMMENTS}?per_page=100&page=2"),
        ]
    );
    assert_eq!(
        setup.github.lines().last().unwrap(),
        &format!("{UPDATE_COMMENT}101")
    );
    assert_eq!(setup.github.comments().len(), 101);
}

// Kills a writer that comments over a newer job of its own section, one that steps aside for an
// older job, and a section without its `--ci-id` label.
#[tokio::test]
async fn a_newer_job_of_a_section_holds_it() {
    let with_ci_id = |ci_id: &str| {
        let mut github = github(Some(NUMBER));
        github["ci_id"] = ci_id.into();
        github
    };
    let setup = Setup::new(Fake::default()).await;
    let older = setup.submit_ok(&with_ci_id("alpha")).await;
    let newer = setup.submit_ok(&with_ci_id("alpha")).await;
    let other = setup.submit_ok(&with_ci_id("beta")).await;
    setup.until_checks(3).await;

    for job in [older, newer, other] {
        setup.end(job, JobStatus::Processed);
    }
    // The newer job first, so the older one finds it.
    for job in [newer, other, older] {
        setup
            .server
            .context()
            .callbacks
            .fire(&setup.log.logger(), setup.job_id(job));
        setup
            .until(|| setup.post(job).state == JobGitHubState::Posted, "posted")
            .await;
    }
    setup.server.drain_callbacks().await;

    let comments = setup.github.comments();
    let [comment] = comments.as_slice() else {
        panic!("{comments:?}");
    };
    for job in [newer, other] {
        let report = setup.report(job);
        assert!(
            comment.body.contains(&report),
            "{report} in {}",
            comment.body
        );
    }
    assert!(
        !comment.body.contains(&setup.report(older)),
        "{}",
        comment.body
    );
    for label in ["(alpha)</a>", "(beta)</a>"] {
        assert!(comment.body.contains(label), "{label} in {}", comment.body);
    }
    let lines = setup.github.lines();
    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    assert_eq!(
        (count(CREATE_COMMENT), count(UPDATE_COMMENT)),
        (1, 1),
        "{lines:?}"
    );
}

/// A server with the GitHub App and a claimed project linked to the repository.
struct Setup {
    server: TestServer,
    log: LogCapture,
    github: FakeGitHub,
    user: TestUser,
    project: TestProject,
}

impl Setup {
    /// A server whose GitHub App is on a GitHub Enterprise Server at the fake's root, with an
    /// Enterprise license on the project's organization if `licensed`.
    async fn new_enterprise(fake: Fake, licensed: bool) -> Self {
        let github = FakeGitHub::start(fake).await;
        let log = LogCapture::default();
        let server = TestServer::new_with_github_enterprise(
            github.base.clone(),
            Clock::System,
            log.logger(),
        )
        .await;
        let (user, org, project) = seed_org(&server).await;
        if licensed {
            server.license_org(&user, &org, PlanLevel::Enterprise).await;
        }
        let setup = Self {
            server,
            log,
            github,
            user,
            project,
        };
        setup.link(INSTALLATION, REPOSITORY);
        setup
    }

    /// The GitHub context of a run on the fake GitHub Enterprise Server.
    fn enterprise_github(&self, number: Option<u64>) -> serde_json::Value {
        serde_json::json!({
            "api_url": format!("{}/api/v3", self.github.base),
            "repository_id": REPOSITORY,
            "head_sha": SHA,
            "number": number,
        })
    }

    /// Drop every organization's license, and return what each held.
    fn unlicense(&self) -> Vec<(i32, Option<String>)> {
        let licenses = schema::organization::table
            .select((schema::organization::id, schema::organization::license))
            .load::<(i32, Option<String>)>(&mut self.server.db_conn())
            .unwrap();
        diesel::update(schema::organization::table)
            .set(schema::organization::license.eq(None::<String>))
            .execute(&mut self.server.db_conn())
            .unwrap();
        licenses
    }

    /// Give back the licenses that `unlicense` dropped.
    fn relicense(&self, licenses: Vec<(i32, Option<String>)>) {
        for (id, license) in licenses {
            diesel::update(schema::organization::table.filter(schema::organization::id.eq(id)))
                .set(schema::organization::license.eq(license))
                .execute(&mut self.server.db_conn())
                .unwrap();
        }
    }

    async fn new(fake: Fake) -> Self {
        let github = FakeGitHub::start(fake).await;
        let log = LogCapture::default();
        let server =
            TestServer::new_with_github(github.base.clone(), Clock::System, log.logger()).await;
        let (user, project) = seed(&server).await;
        let setup = Self {
            server,
            log,
            github,
            user,
            project,
        };
        setup.link(INSTALLATION, REPOSITORY);
        setup
    }

    fn link(&self, installation: u64, repository: u64) -> QueryLinkedRepository {
        self.server.link_repository(
            &self.project,
            &self.user,
            installation,
            repository,
            FULL_NAME,
        )
    }

    async fn submit_ok(&self, github: &serde_json::Value) -> JobUuid {
        let (status, body) = submit(&self.server, &self.user, &self.project, github).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let report: JsonReport = serde_json::from_str(&body).unwrap();
        report.job.unwrap()
    }

    fn job_id(&self, job: JobUuid) -> JobId {
        schema::job::table
            .filter(schema::job::uuid.eq(job))
            .select(schema::job::id)
            .first(&mut self.server.db_conn())
            .unwrap()
    }

    fn report(&self, job: JobUuid) -> String {
        schema::job::table
            .inner_join(schema::report::table)
            .filter(schema::job::uuid.eq(job))
            .select(schema::report::uuid)
            .first::<String>(&mut self.server.db_conn())
            .unwrap()
    }

    fn post(&self, job: JobUuid) -> QueryJobGitHub {
        QueryJobGitHub::get(&mut self.server.db_conn(), self.job_id(job)).unwrap()
    }

    fn linked(&self) -> (u64, LinkedRepositoryStatus) {
        let link = self.stored_link();
        (link.installation_id.into(), link.status)
    }

    fn stored_link(&self) -> QueryLinkedRepository {
        schema::linked_repository::table
            .inner_join(schema::project::table)
            .filter(schema::project::uuid.eq(self.project.uuid))
            .select(QueryLinkedRepository::as_select())
            .first(&mut self.server.db_conn())
            .unwrap()
    }

    fn end(&self, job: JobUuid, status: JobStatus) {
        diesel::update(schema::job::table.filter(schema::job::uuid.eq(job)))
            .set(schema::job::status.eq(status))
            .execute(&mut self.server.db_conn())
            .unwrap();
    }

    /// End the job, fire its post, and wait for every task to settle.
    async fn end_and_fire(&self, job: JobUuid, status: JobStatus) {
        self.fire(job, status);
        self.server.drain_callbacks().await;
    }

    /// End the job and fire its post.
    fn fire(&self, job: JobUuid, status: JobStatus) {
        self.end(job, status);
        self.server
            .context()
            .callbacks
            .fire(&self.log.logger(), self.job_id(job));
    }

    async fn fire_pending(&self) {
        self.server
            .context()
            .callbacks
            .fire_pending(&self.log.logger())
            .await;
    }

    async fn job_json(&self, job: JobUuid) -> serde_json::Value {
        self.server
            .client
            .get(
                self.server
                    .api_url(&format!("/v0/projects/{}/jobs/{job}", self.project.slug)),
            )
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&self.user.token),
            )
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn until_checks(&self, count: usize) {
        self.until(|| self.github.created_checks() >= count, "checks")
            .await;
        // The check is recorded right after GitHub answers.
        self.until(
            || {
                schema::job_github::table
                    .filter(schema::job_github::check_run_id.is_not_null())
                    .count()
                    .get_result::<i64>(&mut self.server.db_conn())
                    .unwrap()
                    >= i64::try_from(count).unwrap()
            },
            "recorded checks",
        )
        .await;
    }

    async fn until_state(&self, job: JobUuid, state: JobGitHubState) {
        self.until(|| self.post(job).state == state, "state").await;
    }

    /// Until the post has counted an attempt, or has settled.
    async fn until_first_attempt(&self, job: JobUuid) {
        self.until(
            || {
                let post = self.post(job);
                post.attempts > 0 || post.state != JobGitHubState::Pending
            },
            "the first attempt",
        )
        .await;
    }

    async fn until_logged(&self, message: &str, field: &str) {
        self.until(|| self.logged(message, &[field]), message).await;
    }

    /// Whether the server logged `message` with every one of `fields`.
    fn logged(&self, message: &str, fields: &[&str]) -> bool {
        self.log.lines().iter().any(|line| {
            line.strip_prefix(message)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
                && fields.iter().all(|field| line.contains(field))
        })
    }

    fn assert_logged(&self, message: &str, fields: &[&str]) {
        assert!(
            self.logged(message, fields),
            "{message} with {fields:?} in {:?}",
            self.log.lines()
        );
    }

    /// Hide a table from the server, or show it again, so its reads fail meanwhile.
    fn rename_table(&self, from: &str, to: &str) {
        diesel::sql_query(format!("ALTER TABLE {from} RENAME TO {to}"))
            .execute(&mut self.server.db_conn())
            .unwrap();
    }

    async fn until<F>(&self, done: F, what: &str)
    where
        F: Fn() -> bool,
    {
        let deadline = Instant::now() + SETTLE;
        while !done() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
        assert!(done(), "{what} after {SETTLE:?}: {:?}", self.github.lines());
    }

    /// The minted token is in no byte of the database, its write-ahead log included, and in no
    /// line the server logged.
    fn assert_no_token(&self) {
        let database = self.server.db_path().display().to_string();
        for path in [database.clone(), format!("{database}-wal")] {
            if let Ok(bytes) = std::fs::read(&path) {
                assert!(
                    !bytes
                        .windows(TOKEN.len())
                        .any(|window| window == TOKEN.as_bytes()),
                    "the token is in {path}"
                );
            }
        }
        let lines = self.log.lines();
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("GitHub post request")),
            "no request in {lines:?}"
        );
        for line in lines {
            assert!(!line.contains(TOKEN), "the token is in {line}");
        }
    }
}

async fn seed(server: &TestServer) -> (TestUser, TestProject) {
    let (user, _org, project) = seed_org(server).await;
    (user, project)
}

async fn seed_org(server: &TestServer) -> (TestUser, TestOrg, TestProject) {
    let user = server.signup("Post User", "post@example.com").await;
    let response = server
        .client
        .post(server.api_url("/v0/specs"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({
            "name": "Post Spec",
            "os": "linux",
            "architecture": "x86_64",
            "cpu": 2,
            "memory": 4_294_967_296i64,
            "disk": 10_737_418_240i64,
            "fallback": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED, "the spec");
    let org = server.create_org(&user, "Post Org").await;
    let project = server.create_project(&user, &org, "Post Project").await;
    (user, org, project)
}

/// The GitHub context of a detached run in the linked repository, on its pull request if any.
fn github(number: Option<u64>) -> serde_json::Value {
    serde_json::json!({
        "repository_id": REPOSITORY,
        "head_sha": SHA,
        "number": number,
    })
}

fn image(project_slug: &str) -> String {
    format!("localhost/{project_slug}@sha256:{}", "a".repeat(64))
}

async fn submit(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
    github: &serde_json::Value,
) -> (StatusCode, String) {
    let slug: &str = project.slug.as_ref();
    submit_job(
        server,
        user,
        slug,
        serde_json::json!({ "image": image(slug), "github": github }),
    )
    .await
}

/// A run of `job`, on a new branch and testbed with a threshold.
async fn submit_job(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
    job: serde_json::Value,
) -> (StatusCode, String) {
    let response = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&run(project_slug, &job))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.text().await.unwrap())
}

/// Submit a run without authentication, which creates `project_slug` in an unclaimed organization.
async fn submit_unclaimed(server: &TestServer, project_slug: &str) -> (StatusCode, String) {
    let mut run = run(
        project_slug,
        &serde_json::json!({ "image": image(project_slug), "github": github(Some(NUMBER)) }),
    );
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
        .unwrap();
    let status = response.status();
    (status, response.text().await.unwrap())
}

fn run(project_slug: &str, job: &serde_json::Value) -> serde_json::Value {
    let new = format!("post-{}", uuid::Uuid::new_v4().simple());
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
                "metric": "post",
                "model": { "test": "percentage", "upper_boundary": 0.05 },
            }],
        },
        "job": job,
    })
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
    .map(Result::unwrap)
}

fn installation_id(id: u64) -> bencher_json::GitHubInstallationId {
    bencher_json::GitHubInstallationId::try_from(id).unwrap()
}

/// A stand-in for GitHub that answers each request by its route, and keeps the checks and the
/// comments it was asked to write.
struct FakeGitHub {
    base: url::Url,
    fake: Arc<Mutex<Fake>>,
    held: Arc<Notify>,
    release: Arc<Notify>,
    task: JoinHandle<()>,
}

/// What the fake answers, and what it was asked.
#[derive(Default)]
struct Fake {
    requests: Vec<Request>,
    checks: usize,
    comments: Vec<Comment>,
    head: Option<String>,
    no_pull_request: bool,
    /// The mint's refusal at an installation: its status and message.
    refusals: Vec<(u64, u16, String)>,
    /// The installation that holds the repository now, if any.
    installation: Option<u64>,
    failures: VecDeque<Failure>,
    hold: Option<String>,
    /// The next request whose line starts with this gets no answer, so its client times out.
    silence: Option<String>,
    /// Where the next lookup of the repository's installation redirects.
    redirect: Option<String>,
    /// The repository's full name, which the mint returns, when it is not `FULL_NAME`.
    renamed: Option<String>,
}

/// The next request whose line starts with `prefix` gets the status and the headers.
struct Failure {
    prefix: String,
    status: u16,
    headers: Headers,
}

/// An answer's headers, each a name and a value.
type Headers = Vec<(String, String)>;

/// When the fake sends an answer.
#[derive(Clone, Copy)]
enum Timing {
    Now,
    /// Once the test calls `release`.
    Released,
    Never,
}

#[derive(Debug, Clone)]
struct Request {
    line: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

#[derive(Debug, Clone)]
struct Comment {
    id: u64,
    body: String,
    login: String,
    kind: String,
    app: Option<String>,
}

impl Fake {
    fn with_head(mut self, head: &str) -> Self {
        self.head = Some(head.to_owned());
        self
    }

    fn without_pull_request(mut self) -> Self {
        self.no_pull_request = true;
        self
    }

    fn renamed(mut self, full_name: &str) -> Self {
        self.renamed = Some(full_name.to_owned());
        self
    }

    fn full_name(&self) -> &str {
        self.renamed.as_deref().unwrap_or(FULL_NAME)
    }

    fn fail(&mut self, prefix: &str, status: u16) {
        self.fail_with(prefix, status, &[]);
    }

    fn fail_with(&mut self, prefix: &str, status: u16, headers: &[(&str, &str)]) {
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        self.failures.push_back(Failure {
            prefix: prefix.to_owned(),
            status,
            headers,
        });
    }

    fn redirect_lookup(&mut self, location: &str) {
        self.redirect = Some(location.to_owned());
    }

    fn hold(&mut self, prefix: &str) {
        self.hold = Some(prefix.to_owned());
    }

    fn silence(&mut self, prefix: &str) {
        self.silence = Some(prefix.to_owned());
    }

    fn refuse_mint(&mut self, installation: u64, status: u16, message: &str) {
        self.refusals
            .push((installation, status, message.to_owned()));
    }

    fn install(&mut self, installation: u64) {
        self.installation = Some(installation);
    }

    fn comment(&mut self, login: &str, kind: &str, app: Option<&str>, body: &str) {
        let id = u64::try_from(self.comments.len()).unwrap() + 1;
        self.comments.push(Comment {
            id,
            body: body.to_owned(),
            login: login.to_owned(),
            kind: kind.to_owned(),
            app: app.map(str::to_owned),
        });
    }

    /// The page of comments a listing asks for, as GitHub lists them.
    fn comments_page(&self, line: &str) -> String {
        let page: usize = line
            .rsplit_once("&page=")
            .and_then(|(_, page)| page.parse().ok())
            .unwrap();
        let comments: Vec<serde_json::Value> = self
            .comments
            .iter()
            .skip((page - 1) * 100)
            .take(100)
            .map(|comment| {
                serde_json::json!({
                    "id": comment.id,
                    "body": comment.body,
                    "user": { "login": comment.login, "type": comment.kind },
                    "performed_via_github_app": comment.app.as_ref().map(|slug| serde_json::json!({ "id": 1, "slug": slug })),
                })
            })
            .collect();
        serde_json::Value::Array(comments).to_string()
    }

    /// The answer to one request, its headers, and when to send it.
    fn answer(&mut self, request: Request) -> (u16, String, Headers, Timing) {
        let line = request.line.clone();
        let body = request.body.clone();
        self.requests.push(request);
        let timing = self.timing(&line);
        if let Some((status, body, headers)) = self.scripted(&line) {
            return (status, body, headers, timing);
        }
        let (method, path) = line.split_once(' ').unwrap();
        let path = path.split('?').next().unwrap();
        let path = path.trim_start_matches("/github/");
        // A GitHub Enterprise Server serves the same routes under its REST API.
        let path = path.strip_prefix("api/v3/").unwrap_or(path);
        let segments: Vec<&str> = path.split('/').collect();
        // Only the lookup still finds the repository by a name it no longer has.
        if let ["repos", owner, name, rest @ ..] = segments.as_slice()
            && *rest != ["installation"]
            && format!("{owner}/{name}") != self.full_name()
        {
            return (
                404,
                r#"{"message": "Not Found"}"#.to_owned(),
                Vec::new(),
                timing,
            );
        }
        let (status, body) = match (method, segments.as_slice()) {
            ("POST", ["app", "installations", installation, "access_tokens"]) => {
                let installation: u64 = installation.parse().unwrap();
                if let Some((_, status, message)) = self
                    .refusals
                    .iter()
                    .find(|(refused, _, _)| *refused == installation)
                {
                    (*status, format!(r#"{{"message": "{message}"}}"#))
                } else {
                    let (owner, name) = self.full_name().split_once('/').unwrap();
                    (
                        201,
                        format!(
                            r#"{{"token": "{TOKEN}", "expires_at": "2024-07-11T07:23:00Z", "repositories": [{{"id": {REPOSITORY}, "name": "{name}", "owner": {{"login": "{owner}"}}}}]}}"#
                        ),
                    )
                }
            },
            ("GET", ["app", "installations", installation]) => (
                200,
                format!(r#"{{"id": {installation}, "html_url": "{APPROVAL_URL}"}}"#),
            ),
            (
                "GET",
                ["repos", "octo-cat", "repo", "installation"] | ["repositories", _, "installation"],
            ) => self.installation.map_or_else(
                || (404, r#"{"message": "Not Found"}"#.to_owned()),
                |id| (200, format!(r#"{{"id": {id}}}"#)),
            ),
            ("POST", ["repos", _, _, "check-runs"]) => {
                self.checks += 1;
                (201, format!(r#"{{"id": {}}}"#, self.checks))
            },
            ("PATCH", ["repos", _, _, "check-runs", id]) => (200, format!(r#"{{"id": {id}}}"#)),
            ("GET", ["repos", _, _, "pulls", _]) => {
                if self.no_pull_request {
                    (404, r#"{"message": "Not Found"}"#.to_owned())
                } else {
                    let head = self.head.as_deref().unwrap_or(SHA);
                    (200, format!(r#"{{"head": {{"sha": "{head}"}}}}"#))
                }
            },
            ("GET", ["repos", _, _, "issues", _, "comments"]) => (200, self.comments_page(&line)),
            ("POST", ["repos", _, _, "issues", _, "comments"]) => {
                self.comment(BOT, "Bot", Some("bencher-test"), &comment_body(&body));
                let id = self.comments.len();
                (201, format!(r#"{{"id": {id}}}"#))
            },
            ("PATCH", ["repos", _, _, "issues", "comments", id]) => {
                let id: u64 = id.parse().unwrap();
                if let Some(comment) = self.comments.iter_mut().find(|comment| comment.id == id) {
                    comment.body = comment_body(&body);
                    (200, format!(r#"{{"id": {id}}}"#))
                } else {
                    (404, r#"{"message": "Not Found"}"#.to_owned())
                }
            },
            _ => (404, r#"{"message": "Not Found"}"#.to_owned()),
        };
        (status, body, Vec::new(), timing)
    }

    /// When to send the answer to the request: a silenced or held request uses up its mark.
    fn timing(&mut self, line: &str) -> Timing {
        if self
            .silence
            .as_deref()
            .is_some_and(|silence| line.starts_with(silence))
        {
            self.silence = None;
            Timing::Never
        } else if self
            .hold
            .as_deref()
            .is_some_and(|hold| line.starts_with(hold))
        {
            self.hold = None;
            Timing::Released
        } else {
            Timing::Now
        }
    }

    /// The answer a test scripted for the request, if any: a failure, or a lookup's redirect.
    fn scripted(&mut self, line: &str) -> Option<(u16, String, Headers)> {
        if let Some(index) = self
            .failures
            .iter()
            .position(|failure| line.starts_with(failure.prefix.as_str()))
        {
            let Failure {
                prefix: _,
                status,
                headers,
            } = self.failures.remove(index)?;
            return Some((status, r#"{"message": "Refused"}"#.to_owned(), headers));
        }
        if !line.starts_with(LOOKUP) {
            return None;
        }
        let location = self.redirect.take()?;
        Some((301, String::new(), vec![("location".to_owned(), location)]))
    }
}

impl FakeGitHub {
    async fn start(fake: Fake) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/github", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let fake = Arc::new(Mutex::new(fake));
        let held = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let task = tokio::spawn({
            let (fake, held, release) =
                (Arc::clone(&fake), Arc::clone(&held), Arc::clone(&release));
            async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let request = read_request(&mut stream).await;
                    let (status, body, headers, timing) = fake.lock().unwrap().answer(request);
                    match timing {
                        Timing::Now => {},
                        Timing::Released => {
                            held.notify_one();
                            release.notified().await;
                        },
                        Timing::Never => {
                            // Until the client times out and hangs up.
                            drop(stream.read_to_end(&mut Vec::new()).await);
                            continue;
                        },
                    }
                    let headers = headers
                        .iter()
                        .fold(String::new(), |mut lines, (name, value)| {
                            lines.push_str(name);
                            lines.push_str(": ");
                            lines.push_str(value);
                            lines.push_str("\r\n");
                            lines
                        });
                    let response = format!(
                        "HTTP/1.1 {status} Status\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{headers}connection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                    stream.shutdown().await.unwrap();
                }
            }
        });
        Self {
            base,
            fake,
            held,
            release,
            task,
        }
    }

    async fn until_held(&self) {
        tokio::time::timeout(SETTLE, self.held.notified())
            .await
            .expect("no request reached the held answer");
    }

    fn release(&self) {
        self.release.notify_one();
    }

    /// The next request whose line starts with `prefix` gets the status and the headers.
    fn fail_with(&self, prefix: &str, status: u16, headers: &[(&str, &str)]) {
        self.fake.lock().unwrap().fail_with(prefix, status, headers);
    }

    /// The next request whose line starts with `prefix` gets no answer.
    fn silence(&self, prefix: &str) {
        self.fake.lock().unwrap().silence(prefix);
    }

    fn requests(&self) -> Vec<Request> {
        self.fake.lock().unwrap().requests.clone()
    }

    /// Each request's method and path.
    fn lines(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|request| {
                let line = request.line;
                line.split('?').next().unwrap().to_owned()
            })
            .collect()
    }

    fn created_checks(&self) -> usize {
        self.fake.lock().unwrap().checks
    }

    fn comments(&self) -> Vec<Comment> {
        self.fake.lock().unwrap().comments.clone()
    }
}

impl Drop for FakeGitHub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn comment_body(body: &serde_json::Value) -> String {
    body.get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_owned()
}

async fn read_request(stream: &mut TcpStream) -> Request {
    let mut buffer = Vec::new();
    let mut chunk = [0; 4096];
    let head_len = loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "the client closed before the request head ended");
        buffer.extend_from_slice(chunk.get(..read).unwrap());
    };
    let mut body = buffer.split_off(head_len);
    let head = String::from_utf8(buffer).unwrap();
    let header = |name: &str| {
        head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length").map_or(0, |length| length.parse().unwrap());
    while body.len() < length {
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "the client closed before the request body ended");
        body.extend_from_slice(chunk.get(..read).unwrap());
    }
    let line = head.lines().next().unwrap();
    let line = line.strip_suffix(" HTTP/1.1").unwrap_or(line).to_owned();
    Request {
        line,
        authorization: header("authorization"),
        body: serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    }
}
