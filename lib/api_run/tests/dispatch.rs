#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the GitHub Actions dispatch without a token, whose token Bencher mints
//! for the project's linked repository when the callback fires.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bencher_api_tests::{
    LogCapture, TestProject, TestServer, TestUser,
    github::{Answer, GitHubServer, HOLD, NO_ANSWER, Received},
};
use bencher_json::{
    BENCHER_API_VERSION, Clock, JobStatus, JobUuid, JsonReport,
    project::github::{LinkedRepositoryReason, LinkedRepositoryStatus},
    runner::JobCallbackState,
};
use bencher_schema::{
    model::{
        project::linked_repository::QueryLinkedRepository,
        runner::{JobId, QueryJobCallback},
    },
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use http::StatusCode;

const INSTALLATION: u64 = 2048;
const MOVED_INSTALLATION: u64 = 8192;
const REPOSITORY: u64 = 4096;
const FULL_NAME: &str = "octo-cat/repo-old";
const MINTED_NAME: &str = "repo-new";
const TOKEN: &str = "ghs_minted-token-marker";
const STORED_TOKEN: &str = "github_pat_stored-token-marker";
const STORED_URL: &str = "https://api.github.com/repos/someone/elsewhere/dispatches";
const HEADER_MARKER: &str = "callback-header-marker";
const REPOSITORY_INACCESSIBLE: &str = "There is at least one repository that does not exist or is not accessible to the parent installation.";
const PERMISSIONS_NOT_GRANTED: &str =
    "The permissions requested are not granted to this installation.";
const MINT: &str = "POST /github/app/installations/2048/access_tokens HTTP/1.1";
const LOOKUP: &str = "GET /github/repos/octo-cat/repo-old/installation HTTP/1.1";
const MINT_MOVED: &str = "POST /github/app/installations/8192/access_tokens HTTP/1.1";
const MOVED: &str = "/github/repositories/4096/installation";
const OTHER_REPOSITORY: u64 = 4097;
const OTHER_NAME: &str = "repo-other";
/// Long enough for a retry after a short pause, and well short of a fallback pause.
const SETTLE: Duration = Duration::from_secs(30);

struct Setup {
    server: TestServer,
    log: LogCapture,
    github: Option<GitHubServer>,
    /// Where the app sends every GitHub request.
    base: String,
    user: TestUser,
    project: TestProject,
}

impl Setup {
    fn github(&self) -> &GitHubServer {
        self.github.as_ref().expect("GitHub's requests were read")
    }

    /// Every request GitHub got, once it has answered all of its scripted responses.
    async fn received(&mut self) -> Vec<Received> {
        self.github
            .take()
            .expect("GitHub's requests are read once")
            .received()
            .await
    }
}

/// A server with the GitHub App, whose GitHub answers with `responses` in turn, and a claimed
/// project with no link.
async fn setup(responses: Vec<Answer>) -> Setup {
    let github = GitHubServer::start_with_headers(responses).await;
    let log = LogCapture::default();
    let base = github.base();
    let server = TestServer::new_with_github(base.clone(), Clock::System, log.logger()).await;
    let (user, project) = seed(&server).await;
    Setup {
        server,
        log,
        github: Some(github),
        base: base.to_string(),
        user,
        project,
    }
}

// Each run names a new branch and testbed, with a threshold. Kills each refusal at submit, a
// refusal of a dispatch with its own token or of one for an unclaimed project, a refusal of a
// connected link, a refusal that comes after the run creates its branch, testbed, threshold,
// report, or job, and a refusal that never names `--ci-callback-token`.
#[tokio::test]
async fn a_dispatch_that_could_never_get_a_token_is_refused_at_submit() {
    let server = TestServer::new().await;
    let (user, project) = seed(&server).await;
    let before = created(&server);

    let (status, body) = submit(&server, &user, project.slug.as_ref(), &minted_dispatch()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("`--ci-callback-token`"), "{body}");
    assert_eq!(created(&server), before, "no app");
    let (status, body) = submit(&server, &user, project.slug.as_ref(), &bearer_dispatch()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut setup = setup(Vec::new()).await;
    let (server, user, project) = (&setup.server, &setup.user, &setup.project);
    let slug: &str = project.slug.as_ref();
    let before = created(server);

    let (status, body) = submit(server, user, slug, &minted_dispatch()).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    for named in ["`--ci-callback-token`", "Settings", slug] {
        assert!(body.contains(named), "{named} not in {body}");
    }
    assert_eq!(created(server), before, "no link");
    let (status, body) = submit(server, user, slug, &bearer_dispatch()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let link = server.link_repository(project, user, INSTALLATION, REPOSITORY, FULL_NAME);
    link.disconnect(&mut server.db_conn(), LinkedRepositoryReason::Removed)
        .expect("Failed to disconnect the link");
    let before = created(server);

    let (status, body) = submit(server, user, slug, &minted_dispatch()).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    for named in ["`--ci-callback-token`", "Settings", FULL_NAME] {
        assert!(body.contains(named), "{named} not in {body}");
    }
    assert_eq!(created(server), before, "a disconnected link");

    server.link_repository(project, user, INSTALLATION, REPOSITORY, FULL_NAME);
    let (status, body) = submit(server, user, slug, &minted_dispatch()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        callback(server, run_job(&body)).state,
        JobCallbackState::Pending
    );

    // An unclaimed project has no link to make, so its dispatch is skipped as any other is.
    let unclaimed = "dispatch-unclaimed-project";
    let job = submit_unclaimed(server, unclaimed, &minted_dispatch()).await;
    assert_eq!(callback(server, job).state, JobCallbackState::Skipped);
    assert_eq!(
        setup.received().await.len(),
        0,
        "a submit never calls GitHub"
    );
}

// Kills a dispatch sent to the stored URL's owner and name, with a stored header, without the
// minted token or with any other, a full name never written back, a mint for any other
// installation or repository, and a minted token that reaches a stored row or a log line, whether
// GitHub takes the dispatch or refuses it.
#[tokio::test]
async fn a_minted_dispatch_goes_to_the_minted_repository_and_leaves_no_token() {
    for (answer, state) in [
        (StatusCode::NO_CONTENT, JobCallbackState::Delivered),
        (StatusCode::NOT_FOUND, JobCallbackState::Failed),
    ] {
        let mut setup = setup(vec![minted(MINTED_NAME)]).await;
        setup.server.link_repository(
            &setup.project,
            &setup.user,
            INSTALLATION,
            REPOSITORY,
            FULL_NAME,
        );
        let job = submit_ok(&setup, &minted_dispatch()).await;
        setup.server.answer_next_callback(answer);

        fire(&setup, job).await;

        let [request] = setup
            .server
            .callback_requests()
            .try_into()
            .ok()
            .expect("one dispatch");
        assert_eq!(
            request.url.as_str(),
            format!("{}/repos/octo-cat/{MINTED_NAME}/dispatches", setup.base)
        );
        let headers: BTreeMap<&str, &str> = request
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.to_str().expect("Invalid header")))
            .collect();
        let bearer = format!("Bearer {TOKEN}");
        let user_agent = format!(
            "Bencher/{BENCHER_API_VERSION} (+https://bencher.dev/docs/explanation/images/)"
        );
        assert_eq!(
            headers,
            BTreeMap::from([
                ("accept", "application/vnd.github+json"),
                ("authorization", bearer.as_str()),
                ("content-type", "application/json"),
                ("user-agent", user_agent.as_str()),
                ("x-github-api-version", "2022-11-28"),
            ]),
            "{answer}"
        );
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("Invalid dispatch body");
        assert_eq!(body["event_type"], "bencher_run");
        assert_eq!(
            body["client_payload"]["bencher"]["job"],
            job.to_string().as_str()
        );
        let [mint]: [Received; 1] = setup.received().await.try_into().ok().expect("one mint");
        assert_mint(&mint, INSTALLATION);
        assert_eq!(
            linked(&setup),
            (
                INSTALLATION,
                format!("octo-cat/{MINTED_NAME}"),
                LinkedRepositoryStatus::Connected,
                None
            ),
            "{answer}"
        );
        assert_eq!(settled(&setup, job), (state, 1, Some(answer.as_u16())));
        assert_no_token(&setup);
    }
}

// Kills a mint for a dispatch with its own token, and sending one anywhere, with any token, or
// with any headers but those it was stored with.
#[tokio::test]
async fn a_dispatch_with_its_own_token_is_sent_as_stored_on_a_linked_project() {
    let mut setup = setup(Vec::new()).await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let job = submit_ok(&setup, &bearer_dispatch()).await;

    fire(&setup, job).await;

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
    assert_eq!(setup.received().await.len(), 0, "no mint");
    assert_eq!(
        linked(&setup),
        (
            INSTALLATION,
            FULL_NAME.to_owned(),
            LinkedRepositoryStatus::Connected,
            None
        )
    );
    assert_eq!(
        settled(&setup, job),
        (JobCallbackState::Delivered, 1, Some(200))
    );
}

// A refusal that says the repository left its installation, and a lookup that finds the one that
// holds it now. Kills a lookup that finds one and does not move the link, connect it, or mint once
// more there, and a full name not written back after a move.
#[tokio::test]
async fn a_refusal_for_a_moved_repository_moves_the_link() {
    let minted_name = format!("octo-cat/{MINTED_NAME}");
    for refusal_row in [
        Refusal {
            what: "404, then an installation",
            answers: vec![
                refusal(404, "Not Found"),
                installation(MOVED_INSTALLATION),
                minted(MINTED_NAME),
            ],
            requests: vec![MINT, LOOKUP, MINT_MOVED],
            link: linked_to(MOVED_INSTALLATION, &minted_name, None),
            callback: delivered(1),
            stages: vec!["send"],
        },
        Refusal {
            what: "422 not accessible, then an installation",
            answers: vec![
                refusal(422, REPOSITORY_INACCESSIBLE),
                installation(MOVED_INSTALLATION),
                minted(MINTED_NAME),
            ],
            requests: vec![MINT, LOOKUP, MINT_MOVED],
            link: linked_to(MOVED_INSTALLATION, &minted_name, None),
            callback: delivered(1),
            stages: vec!["send"],
        },
    ] {
        assert_refusal(refusal_row).await;
    }
}

// A renamed or transferred repository's lookup answers with a redirect. Kills a redirect not
// followed on the API's origin.
#[tokio::test]
async fn a_lookup_redirected_on_the_api_origin_moves_the_link() {
    assert_refusal(Refusal {
        what: "404, then a redirect to an installation",
        answers: vec![
            refusal(404, "Not Found"),
            redirect(MOVED),
            installation(MOVED_INSTALLATION),
            minted(MINTED_NAME),
        ],
        requests: vec![
            MINT,
            LOOKUP,
            "GET /github/repositories/4096/installation HTTP/1.1",
            MINT_MOVED,
        ],
        link: linked_to(MOVED_INSTALLATION, &format!("octo-cat/{MINTED_NAME}"), None),
        callback: delivered(1),
        stages: vec!["send"],
    })
    .await;
}

// A refusal that says the repository left its installation, and no installation that holds it.
// Kills a lookup with no installation that leaves the link connected or gives it the wrong reason,
// and a refusal that the callback does not count or record. A repository that an installation for
// all repositories drops when it changes to selected ones sends no webhook event, so the 422 with
// no installation is its only way to disconnect.
#[tokio::test]
async fn a_refusal_for_a_repository_no_installation_holds_disconnects_the_link() {
    for refusal_row in [
        Refusal {
            what: "404, then none",
            answers: vec![refusal(404, "Not Found"), refusal(404, "Not Found")],
            requests: vec![MINT, LOOKUP],
            link: linked_to(
                INSTALLATION,
                FULL_NAME,
                Some(LinkedRepositoryReason::Uninstalled),
            ),
            callback: failed(404),
            stages: vec!["mint"],
        },
        Refusal {
            what: "422 not accessible, then none",
            answers: vec![
                refusal(422, REPOSITORY_INACCESSIBLE),
                refusal(404, "Not Found"),
            ],
            requests: vec![MINT, LOOKUP],
            link: linked_to(
                INSTALLATION,
                FULL_NAME,
                Some(LinkedRepositoryReason::Removed),
            ),
            callback: failed(422),
            stages: vec!["mint"],
        },
    ] {
        assert_refusal(refusal_row).await;
    }
}

// Kills a second refusal that leaves the moved link connected.
#[tokio::test]
async fn a_moved_link_refused_again_disconnects() {
    assert_refusal(Refusal {
        what: "404, then an installation that refuses too",
        answers: vec![
            refusal(404, "Not Found"),
            installation(MOVED_INSTALLATION),
            refusal(404, "Not Found"),
        ],
        requests: vec![MINT, LOOKUP, MINT_MOVED],
        link: linked_to(
            MOVED_INSTALLATION,
            FULL_NAME,
            Some(LinkedRepositoryReason::Uninstalled),
        ),
        callback: failed(404),
        stages: vec!["mint"],
    })
    .await;
}

// Kills a lookup after any other refusal, and an attempt that does not say that the mint ended
// it.
#[tokio::test]
async fn any_other_refusal_leaves_the_link() {
    for refusal_row in [
        Refusal {
            what: "422 permissions not granted",
            answers: vec![refusal(422, PERMISSIONS_NOT_GRANTED)],
            requests: vec![MINT],
            link: linked_to(INSTALLATION, FULL_NAME, None),
            callback: failed(422),
            stages: vec!["mint"],
        },
        Refusal {
            what: "403",
            answers: vec![refusal(403, "This installation has been suspended")],
            requests: vec![MINT],
            link: linked_to(INSTALLATION, FULL_NAME, None),
            callback: failed(403),
            stages: vec!["mint"],
        },
    ] {
        assert_refusal(refusal_row).await;
    }
}

// Kills a failed lookup that disconnects the link or ends a callback that a retry can still
// deliver, a redirect followed off the API's origin, and an attempt that does not say that the
// lookup ended it.
#[tokio::test]
async fn a_failed_lookup_leaves_the_link() {
    for refusal_row in [
        Refusal {
            what: "404, then a failed lookup, then a mint",
            answers: vec![
                refusal(404, "Not Found"),
                refusal(500, "Server Error"),
                minted(MINTED_NAME),
            ],
            requests: vec![MINT, LOOKUP, MINT],
            link: linked_to(INSTALLATION, &format!("octo-cat/{MINTED_NAME}"), None),
            callback: delivered(2),
            stages: vec!["lookup", "send"],
        },
        Refusal {
            what: "404, then a redirect off the API's origin",
            answers: vec![
                refusal(404, "Not Found"),
                redirect("https://elsewhere.example/repositories/4096/installation"),
            ],
            requests: vec![MINT, LOOKUP],
            link: linked_to(INSTALLATION, FULL_NAME, None),
            callback: failed(301),
            stages: vec!["lookup"],
        },
    ] {
        assert_refusal(refusal_row).await;
    }
}

// The pacer's rules for a dispatch apply to the mint. Kills a 5xx from the mint that ends the
// callback.
#[tokio::test]
async fn a_mint_server_error_is_retried() {
    assert_retried("500", refusal(500, "Server Error"), "5xx").await;
}

// Kills a mint's 429 whose `retry-after` never reaches the pacer, which would then wait out a
// minute's fallback pause.
#[tokio::test]
async fn a_rate_limited_mint_is_retried_after_its_retry_after() {
    assert_retried(
        "429 with retry-after",
        with_headers(
            refusal(429, "API rate limit exceeded"),
            vec![("retry-after", "1".to_owned())],
        ),
        "4xx",
    )
    .await;
}

// Kills a mint's 403 whose rate-limit headers never reach the pacer, which would then end the
// callback.
#[tokio::test]
async fn a_mint_with_no_requests_remaining_is_retried_at_its_reset() {
    let reset = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Invalid clock")
        .as_secs()
        + 1;
    assert_retried(
        "403 with no requests remaining",
        with_headers(
            refusal(403, "API rate limit exceeded"),
            vec![
                ("x-ratelimit-remaining", "0".to_owned()),
                ("x-ratelimit-reset", reset.to_string()),
            ],
        ),
        "4xx",
    )
    .await;
}

// Kills a mint timeout that ends the callback or is classed as a connection error.
#[tokio::test]
async fn a_mint_timeout_is_retried() {
    assert_retried("timeout", (NO_ANSWER, Vec::new(), String::new()), "timeout").await;
}

// Kills pacing a minted dispatch by anything but its installation, which would hold every other
// installation's dispatches behind one installation's rate limit. The 429's `retry-after` pauses
// its own installation for longer than the test waits.
#[tokio::test]
async fn a_rate_limit_pauses_only_its_installation() {
    let mut setup = setup(vec![
        with_headers(
            refusal(429, "API rate limit exceeded"),
            vec![("retry-after", "60".to_owned())],
        ),
        minted_for(OTHER_REPOSITORY, OTHER_NAME),
    ])
    .await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let org = setup
        .server
        .create_org(&setup.user, "Other Dispatch Org")
        .await;
    let other = setup
        .server
        .create_project(&setup.user, &org, "Other Project")
        .await;
    setup.server.link_repository(
        &other,
        &setup.user,
        MOVED_INSTALLATION,
        OTHER_REPOSITORY,
        "octo-cat/repo-other",
    );
    let limited = submit_ok(&setup, &minted_dispatch()).await;
    let (status, body) = submit(
        &setup.server,
        &setup.user,
        other.slug.as_ref(),
        &minted_dispatch(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let unlimited = run_job(&body);

    end_and_fire(&setup, limited);
    until(&setup, limited, SETTLE, |(_, attempts, _)| attempts == 1).await;
    end_and_fire(&setup, unlimited);
    settle_within(&setup, unlimited, Duration::from_secs(10)).await;

    assert_eq!(
        settled(&setup, unlimited),
        (JobCallbackState::Delivered, 1, Some(200))
    );
    assert_eq!(
        settled(&setup, limited),
        (JobCallbackState::Pending, 1, Some(429))
    );
    let received = setup.received().await;
    let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
    assert_eq!(lines, [MINT, MINT_MOVED]);
    let Setup { server, .. } = setup;
    server.close().await;
}

// A lookup holds its answer while the project's link is replaced, so the move finds no link to
// move. Kills a mint once more for the link the attempt read, which the project no longer has,
// and a move or a disconnect that lands on the link that replaced it.
#[tokio::test]
async fn a_link_replaced_during_a_lookup_is_left_alone() {
    let mut setup = setup(vec![
        refusal(404, "Not Found"),
        with_headers(
            installation(MOVED_INSTALLATION),
            vec![(HOLD, String::new())],
        ),
    ])
    .await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let job = submit_ok(&setup, &minted_dispatch()).await;

    end_and_fire(&setup, job);
    setup.github().until_held().await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        OTHER_REPOSITORY,
        "octo-cat/repo-other",
    );
    setup.github().release();
    setup.server.drain_callbacks().await;

    let received = setup.received().await;
    let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
    assert_eq!(lines, [MINT, LOOKUP]);
    assert_eq!(
        settled(&setup, job),
        (JobCallbackState::Failed, 1, Some(404))
    );
    let link = stored(&setup.server, &setup.project);
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
    assert_eq!(setup.server.callback_requests().len(), 0);
}

// Kills reading the link once for every attempt: a link replaced after a refused attempt changes
// what the next attempt mints for.
#[tokio::test]
async fn the_next_attempt_mints_for_a_replaced_link() {
    change_the_link_after_a_refused_attempt(LinkChange::Replaced).await;
}

// Kills reading the link once for every attempt: a link removed after a refused attempt ends the
// callback before it mints again.
#[tokio::test]
async fn the_next_attempt_mints_nothing_for_a_removed_link() {
    change_the_link_after_a_refused_attempt(LinkChange::Removed).await;
}

// A refusal that leaves the link as it is asks nothing more of GitHub, so the next job's mint
// takes the answer that a lookup would have taken. Kills a lookup, and with it a move or a
// disconnect, after a 422 for permissions not granted or a 403.
#[tokio::test]
async fn a_refusal_that_keeps_the_link_looks_up_nothing() {
    for (what, refused, status) in [
        (
            "422 permissions not granted",
            refusal(422, PERMISSIONS_NOT_GRANTED),
            422,
        ),
        (
            "403",
            refusal(403, "This installation has been suspended"),
            403,
        ),
    ] {
        let mut setup = setup(vec![refused, minted(MINTED_NAME)]).await;
        setup.server.link_repository(
            &setup.project,
            &setup.user,
            INSTALLATION,
            REPOSITORY,
            FULL_NAME,
        );
        let refused_job = submit_ok(&setup, &minted_dispatch()).await;
        let next_job = submit_ok(&setup, &minted_dispatch()).await;

        end_and_fire(&setup, refused_job);
        settle_within(&setup, refused_job, SETTLE).await;
        fire(&setup, next_job).await;

        let received = setup.received().await;
        let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
        assert_eq!(lines, [MINT, MINT], "{what}");
        assert_eq!(
            settled(&setup, refused_job),
            (JobCallbackState::Failed, 1, Some(status)),
            "{what}"
        );
        assert_eq!(
            settled(&setup, next_job),
            (JobCallbackState::Delivered, 1, Some(200)),
            "{what}"
        );
        assert_eq!(
            linked(&setup),
            (
                INSTALLATION,
                format!("octo-cat/{MINTED_NAME}"),
                LinkedRepositoryStatus::Connected,
                None
            ),
            "{what}"
        );
    }
}

// Kills sending a dispatch without a token as stored, or minting one, for a project whose link
// was removed or disconnected after the submit.
#[tokio::test]
async fn a_link_gone_before_the_fire_sends_nothing() {
    for remove in [true, false] {
        let mut setup = setup(Vec::new()).await;
        let link = setup.server.link_repository(
            &setup.project,
            &setup.user,
            INSTALLATION,
            REPOSITORY,
            FULL_NAME,
        );
        let job = submit_ok(&setup, &minted_dispatch()).await;
        let mut conn = setup.server.db_conn();
        if remove {
            QueryLinkedRepository::delete(&mut conn, link.project_id)
                .expect("Failed to remove the link");
        } else {
            link.disconnect(&mut conn, LinkedRepositoryReason::Uninstalled)
                .expect("Failed to disconnect the link");
        }

        fire(&setup, job).await;

        assert_eq!(setup.server.callback_requests().len(), 0, "{remove}");
        assert_eq!(setup.received().await.len(), 0, "{remove}");
        assert_eq!(
            settled(&setup, job),
            (JobCallbackState::Failed, 0, None),
            "{remove}"
        );
        let unsent: Vec<String> = setup
            .log
            .lines()
            .into_iter()
            .filter(|line| line.starts_with("Callback request not sent"))
            .collect();
        assert!(
            unsent.iter().any(|line| line.contains("stage=link")),
            "{unsent:?}"
        );
    }
}

// Kills a move that lands on another project's link or on a link replaced since, that matches by
// status, or that leaves the moved link disconnected.
#[tokio::test]
async fn a_move_touches_only_the_link_it_read() {
    let Setup {
        server,
        user,
        project,
        ..
    } = setup(Vec::new()).await;
    let org = server.create_org(&user, "Other Dispatch Org").await;
    let other = server.create_project(&user, &org, "Other Project").await;
    let read = server.link_repository(&project, &user, INSTALLATION, REPOSITORY, FULL_NAME);
    server.link_repository(&other, &user, INSTALLATION, REPOSITORY, FULL_NAME);
    let mut conn = server.db_conn();
    read.disconnect(&mut conn, LinkedRepositoryReason::Removed)
        .expect("Failed to disconnect the link");
    let moved_installation = installation_id(MOVED_INSTALLATION);

    let moved = read
        .move_to(&mut conn, moved_installation)
        .expect("Failed to move the link")
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
    assert_eq!(state(&moved), connected(MOVED_INSTALLATION, REPOSITORY));
    assert_eq!(
        state(&stored(&server, &project)),
        connected(MOVED_INSTALLATION, REPOSITORY)
    );
    assert_eq!(
        state(&stored(&server, &other)),
        connected(INSTALLATION, REPOSITORY),
        "another project's link"
    );

    let replaced = read
        .move_to(&mut conn, installation_id(6000))
        .expect("Failed to move the link");
    assert!(replaced.is_none(), "a link on another installation since");
    assert_eq!(
        state(&stored(&server, &project)),
        connected(MOVED_INSTALLATION, REPOSITORY)
    );

    server.link_repository(&project, &user, INSTALLATION, 4097, "octo-cat/repo-other");
    let replaced = read
        .move_to(&mut conn, installation_id(6000))
        .expect("Failed to move the link");
    assert!(replaced.is_none(), "a link to another repository since");
    assert_eq!(
        state(&stored(&server, &project)),
        connected(INSTALLATION, 4097)
    );
}

/// One refused mint on a link to installation 2048: GitHub's answers, the requests it gets, and
/// how the link and the callback end.
struct Refusal {
    what: &'static str,
    answers: Vec<Answer>,
    requests: Vec<&'static str>,
    link: (
        u64,
        String,
        LinkedRepositoryStatus,
        Option<LinkedRepositoryReason>,
    ),
    callback: (JobCallbackState, i32, Option<u16>),
    /// Where each attempt ended.
    stages: Vec<&'static str>,
}

async fn assert_refusal(refusal_row: Refusal) {
    let Refusal {
        what,
        answers,
        requests,
        link,
        callback,
        stages,
    } = refusal_row;
    let mut setup = setup(answers).await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let job = submit_ok(&setup, &minted_dispatch()).await;

    fire(&setup, job).await;

    let received = setup.received().await;
    let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
    assert_eq!(lines, requests, "{what}");
    for request in &received {
        let authorization = request.header("authorization").unwrap_or_default();
        assert!(authorization.starts_with("Bearer ey"), "{what}: app JWT");
    }
    assert_eq!(linked(&setup), link, "{what}");
    assert_eq!(settled(&setup, job), callback, "{what}");
    let attempted: Vec<String> = attempts(&setup, job)
        .into_iter()
        .map(|(stage, _outcome)| stage)
        .collect();
    assert_eq!(attempted, stages, "{what}");
    let sent: Vec<String> = setup
        .server
        .callback_requests()
        .iter()
        .map(|request| request.url.to_string())
        .collect();
    let expected: Vec<String> = if callback.0 == JobCallbackState::Delivered {
        vec![format!(
            "{}/repos/octo-cat/{MINTED_NAME}/dispatches",
            setup.base
        )]
    } else {
        Vec::new()
    };
    assert_eq!(sent, expected, "{what}");
    assert_no_token(&setup);
}

/// A mint that GitHub answers with `first` and then mints, so the callback is delivered at its
/// second attempt, whose first attempt the mint ended with `outcome`.
async fn assert_retried(what: &str, first: Answer, outcome: &str) {
    let mut setup = setup(vec![first, minted(MINTED_NAME)]).await;
    setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let job = submit_ok(&setup, &minted_dispatch()).await;

    end_and_fire(&setup, job);
    settle_within(&setup, job, SETTLE).await;

    assert_eq!(
        settled(&setup, job),
        (JobCallbackState::Delivered, 2, Some(200)),
        "{what}"
    );
    assert_eq!(
        attempts(&setup, job),
        [
            ("mint".to_owned(), outcome.to_owned()),
            ("send".to_owned(), "2xx".to_owned())
        ],
        "{what}"
    );
    let received = setup.received().await;
    let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
    assert_eq!(lines, [MINT, MINT], "{what}");
    setup.server.drain_callbacks().await;
}

/// Refuse the first mint with a 500, then change the link before the retry.
async fn change_the_link_after_a_refused_attempt(change: LinkChange) {
    let mut responses = vec![refusal(500, "Server Error")];
    if change == LinkChange::Replaced {
        responses.push(minted_for(OTHER_REPOSITORY, OTHER_NAME));
    }
    let mut setup = setup(responses).await;
    let link = setup.server.link_repository(
        &setup.project,
        &setup.user,
        INSTALLATION,
        REPOSITORY,
        FULL_NAME,
    );
    let job = submit_ok(&setup, &minted_dispatch()).await;

    end_and_fire(&setup, job);
    until(&setup, job, SETTLE, |(_, attempts, _)| attempts == 1).await;
    if change == LinkChange::Replaced {
        setup.server.link_repository(
            &setup.project,
            &setup.user,
            MOVED_INSTALLATION,
            OTHER_REPOSITORY,
            "octo-cat/repo-other",
        );
    } else {
        QueryLinkedRepository::delete(&mut setup.server.db_conn(), link.project_id)
            .expect("Failed to remove the link");
    }
    setup.server.drain_callbacks().await;

    let received = setup.received().await;
    let lines: Vec<&str> = received.iter().map(Received::request_line).collect();
    let sent: Vec<String> = setup
        .server
        .callback_requests()
        .iter()
        .map(|request| request.url.to_string())
        .collect();
    if change == LinkChange::Replaced {
        assert_eq!(lines, [MINT, MINT_MOVED], "the mints");
        assert_eq!(
            received.get(1).map(Received::json),
            Some(serde_json::json!({
                "repository_ids": [OTHER_REPOSITORY],
                "permissions": { "contents": "write" },
            })),
            "the second mint"
        );
        assert_eq!(
            sent,
            [format!(
                "{}/repos/octo-cat/{OTHER_NAME}/dispatches",
                setup.base
            )],
            "the dispatch"
        );
        assert_eq!(
            settled(&setup, job),
            (JobCallbackState::Delivered, 2, Some(200)),
            "the callback"
        );
    } else {
        assert_eq!(lines, [MINT], "the mints");
        assert_eq!(sent, Vec::<String>::new(), "no dispatch");
        assert_eq!(
            settled(&setup, job),
            (JobCallbackState::Failed, 1, Some(500)),
            "the callback"
        );
        assert!(
            setup
                .log
                .lines()
                .iter()
                .any(|line| line.starts_with("Callback request not sent")
                    && line.contains("stage=link")),
            "no stage=link"
        );
    }
}

/// What happens to the link between two attempts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LinkChange {
    Replaced,
    Removed,
}

/// A link's installation, full name, status, and reason, which is disconnected only with a reason.
fn linked_to(
    installation: u64,
    full_name: &str,
    reason: Option<LinkedRepositoryReason>,
) -> (
    u64,
    String,
    LinkedRepositoryStatus,
    Option<LinkedRepositoryReason>,
) {
    let status = if reason.is_some() {
        LinkedRepositoryStatus::Disconnected
    } else {
        LinkedRepositoryStatus::Connected
    };
    (installation, full_name.to_owned(), status, reason)
}

fn delivered(attempts: i32) -> (JobCallbackState, i32, Option<u16>) {
    (JobCallbackState::Delivered, attempts, Some(200))
}

fn failed(status: u16) -> (JobCallbackState, i32, Option<u16>) {
    (JobCallbackState::Failed, 1, Some(status))
}

/// The first user, a server admin, with a fallback spec, and a claimed project of theirs.
async fn seed(server: &TestServer) -> (TestUser, TestProject) {
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
    (user, project)
}

/// The dispatch `bencher run` composes without `--ci-callback-token`, to a repository that the
/// link does not name, with one more header.
fn minted_dispatch() -> serde_json::Value {
    serde_json::json!({
        "url": STORED_URL,
        "headers": {
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "X-Callback-Header": HEADER_MARKER,
        },
        "body": dispatch_body(),
    })
}

/// The dispatch `bencher run` composes with `--ci-callback-token`, with one more header.
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
    run_job(&body)
}

/// Submit a run without authentication, which creates `project_slug` in an unclaimed organization.
async fn submit_unclaimed(
    server: &TestServer,
    project_slug: &str,
    callback: &serde_json::Value,
) -> JobUuid {
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
    let body = response.text().await.expect("Failed to read the body");
    assert_eq!(status, StatusCode::CREATED, "{body}");
    run_job(&body)
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

fn run_job(body: &str) -> JobUuid {
    let report: JsonReport = serde_json::from_str(body).expect("Failed to parse the report");
    report.job.expect("The run has no job")
}

/// End the job, then fire its callback and wait for every delivery to settle, after which the
/// server fires no more.
async fn fire(setup: &Setup, job: JobUuid) {
    end_and_fire(setup, job);
    setup.server.drain_callbacks().await;
}

/// Wait up to `limit` for the job's callback to settle.
async fn settle_within(setup: &Setup, job: JobUuid, limit: Duration) {
    until(setup, job, limit, |(state, _, _)| {
        state != JobCallbackState::Pending
    })
    .await;
}

/// Wait up to `limit` for the job's callback to be `done`.
async fn until<F>(setup: &Setup, job: JobUuid, limit: Duration, done: F)
where
    F: Fn((JobCallbackState, i32, Option<u16>)) -> bool,
{
    let deadline = Instant::now() + limit;
    while !done(settled(setup, job)) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
    assert!(
        done(settled(setup, job)),
        "{:?} after {limit:?}",
        settled(setup, job)
    );
}

/// Each attempt the job's callback made: where it ended, and its outcome.
fn attempts(setup: &Setup, job: JobUuid) -> Vec<(String, String)> {
    let field = |line: &str, key: &str| {
        line.split(&format!(" {key}="))
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or_default()
            .to_owned()
    };
    setup
        .log
        .lines()
        .into_iter()
        .filter(|line| line.starts_with("Callback attempt") && line.contains(&format!("job={job}")))
        .map(|line| (field(&line, "stage"), field(&line, "outcome")))
        .collect()
}

fn end_and_fire(setup: &Setup, job: JobUuid) {
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job)))
        .set(schema::job::status.eq(JobStatus::Canceled))
        .execute(&mut setup.server.db_conn())
        .expect("Failed to end the job");
    setup
        .server
        .context()
        .callbacks
        .fire(&setup.log.logger(), job_id(&setup.server, job));
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

/// The callback's state, attempt count, and last status.
fn settled(setup: &Setup, job: JobUuid) -> (JobCallbackState, i32, Option<u16>) {
    let callback = callback(&setup.server, job);
    (
        callback.state,
        callback.attempts,
        callback.status.map(u16::from),
    )
}

fn stored(server: &TestServer, project: &TestProject) -> QueryLinkedRepository {
    schema::linked_repository::table
        .inner_join(schema::project::table)
        .filter(schema::project::uuid.eq(project.uuid))
        .select(QueryLinkedRepository::as_select())
        .first(&mut server.db_conn())
        .expect("No link")
}

/// The project's link: its installation, full name, status, and reason.
fn linked(
    setup: &Setup,
) -> (
    u64,
    String,
    LinkedRepositoryStatus,
    Option<LinkedRepositoryReason>,
) {
    let link = stored(&setup.server, &setup.project);
    (
        link.installation_id.into(),
        link.full_name.to_string(),
        link.status,
        link.reason,
    )
}

fn installation_id(id: u64) -> bencher_json::GitHubInstallationId {
    id.try_into().expect("Invalid installation")
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

/// A mint for the linked repository alone, with contents write, at `installation`.
fn assert_mint(mint: &Received, installation: u64) {
    assert_eq!(
        mint.request_line(),
        format!("POST /github/app/installations/{installation}/access_tokens HTTP/1.1"),
        "the mint's installation"
    );
    assert_eq!(
        mint.json(),
        serde_json::json!({
            "repository_ids": [REPOSITORY],
            "permissions": { "contents": "write" },
        }),
        "the mint's repository and permissions"
    );
}

/// The minted token is in no byte of the database, its write-ahead log included, and in no line
/// the server logged, which holds every attempt.
fn assert_no_token(setup: &Setup) {
    let database = setup.server.db_path().display().to_string();
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
    let lines = setup.log.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("Callback attempt")),
        "no attempt in {lines:?}"
    );
    for line in &lines {
        assert!(!line.contains(TOKEN), "the token is in {line}");
    }
}

fn minted(name: &str) -> Answer {
    minted_for(REPOSITORY, name)
}

fn minted_for(repository: u64, name: &str) -> Answer {
    (
        201,
        Vec::new(),
        format!(
            r#"{{"token": "{TOKEN}", "expires_at": "2024-07-11T07:23:00Z", "permissions": {{"contents": "write", "metadata": "read"}}, "repository_selection": "selected", "repositories": [{{"id": {repository}, "name": "{name}", "full_name": "octo-cat/{name}", "owner": {{"login": "octo-cat", "id": 1}}}}]}}"#
        ),
    )
}

fn installation(id: u64) -> Answer {
    (
        200,
        Vec::new(),
        format!(
            r#"{{"id": {id}, "account": {{"login": "octo-cat", "id": 1}}, "repository_selection": "selected", "permissions": {{"contents": "write", "metadata": "read"}}, "suspended_at": null}}"#
        ),
    )
}

fn refusal(status: u16, message: &str) -> Answer {
    (
        status,
        Vec::new(),
        format!(
            r#"{{"message": "{message}", "documentation_url": "https://docs.github.com/rest", "status": "{status}"}}"#
        ),
    )
}

/// A moved repository's answer, which points to `location`.
fn redirect(location: &str) -> Answer {
    (
        301,
        vec![("location", location.to_owned())],
        r#"{"message": "Moved Permanently"}"#.to_owned(),
    )
}

fn with_headers(answer: Answer, headers: Vec<(&'static str, String)>) -> Answer {
    let (status, _, body) = answer;
    (status, headers, body)
}
