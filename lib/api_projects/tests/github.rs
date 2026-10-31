#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Integration tests for a project's linked GitHub repository.

use bencher_api_tests::{
    LogCapture, TestOrg, TestProject, TestServer, TestUser,
    github::{
        GITHUB_APP_SLUG, GITHUB_CLIENT_SECRET, GITHUB_WEBHOOK_SECRET, GitHubServer, Received,
        github_signature,
    },
    helpers::grant_project_role,
};
use std::sync::Arc;

use bencher_json::{
    Clock, DateTime, GitHubInstallationId, GitHubInstallationPermission, GitHubRepositoryId, Jwt,
    project::github::{
        JsonGitHubLinkUrls, JsonGitHubRepositories, JsonLinkedRepository, JsonProjectGitHub,
        LinkedRepositoryReason, LinkedRepositoryStatus,
    },
};
use bencher_rbac::project::Role;
use bencher_schema::{model::project::linked_repository::QueryLinkedRepository, schema};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use http::StatusCode;
use reqwest::Method;

/// Long past, so only the server's frozen clock accepts a token issued then.
const NOW: i64 = 1_789_344_000;
const TTL: i64 = 10 * 60;
const INSTALLATION: u64 = 2048;
const REPOSITORY: u64 = 4096;
const CODE: &str = "code-marker";
const USER_TOKEN: &str = "user-token-marker";
const RETURN_URL: &str = "http://localhost:3000/console/github";

struct Setup {
    server: TestServer,
    log: LogCapture,
    github: GitHubServer,
    /// An organization Leader, who is no server admin.
    leader: TestUser,
    org: TestOrg,
    project: TestProject,
}

/// A server with the GitHub App, whose GitHub answers with `responses` in turn.
async fn setup(responses: Vec<(u16, String)>) -> Setup {
    let github = GitHubServer::start(responses).await;
    let log = LogCapture::default();
    let clock = Clock::Custom(Arc::new(|| at(NOW)));
    let server = TestServer::new_with_github(github.base(), clock, log.logger()).await;
    // The first user is a server admin, who may do anything.
    server.signup("Admin User", "admin@example.com").await;
    let leader = server.signup("Leader User", "leader@example.com").await;
    let org = server.create_org(&leader, "GitHub Org").await;
    let project = server.create_project(&leader, &org, "GitHub Project").await;
    Setup {
        server,
        log,
        github,
        leader,
        org,
        project,
    }
}

// The whole flow: the state the URLs carry exchanges for a grant, and the grant links the
// repository. Kills a state, a grant, or a link for any other user or project; a grant that
// outlives its ten minutes; an install page for any other app; an exchange that drops the return
// page's redirect or the code; a listing that keeps a repository without push or grants one whose
// installation has not approved contents write; and any credential in a log line.
#[tokio::test]
async fn the_link_flow_links_a_pushable_approved_repository() {
    let Setup {
        server,
        log,
        github,
        leader,
        project,
        ..
    } = setup(vec![
        exchanged(),
        installations(&[(INSTALLATION, "write"), (8192, "read")]),
        repositories(&[(REPOSITORY, true), (4097, false)]),
        repositories(&[(9999, true)]),
    ])
    .await;

    let urls: JsonGitHubLinkUrls = ok(request(
        &server,
        Method::GET,
        &github_path(&project, "/urls"),
        &leader,
    ))
    .await;
    let state = query(&urls.authorize, "state");
    let exchange: JsonGitHubRepositories =
        ok(
            request(&server, Method::POST, "/v0/github/exchange", &leader)
                .json(&serde_json::json!({ "code": CODE, "state": state })),
        )
        .await;

    assert_eq!(urls.authorize.host_str(), Some("github.com"));
    assert_eq!(query(&urls.authorize, "redirect_uri"), RETURN_URL);
    assert_eq!(
        urls.install.as_str(),
        format!("https://github.com/apps/{GITHUB_APP_SLUG}/installations/new?state={state}")
    );
    let claims = server
        .token_key()
        .validate_github_state(&state.parse().expect("Invalid state"), at(NOW))
        .expect("Invalid state");
    assert_eq!((claims.sub, claims.project), (leader.uuid, project.uuid));
    assert_eq!(claims.exp, NOW + TTL);
    assert_eq!(exchange.project, project.slug);
    let listed: Vec<(String, u64, GitHubInstallationPermission, bool)> = exchange
        .repositories
        .iter()
        .map(|repository| {
            (
                repository.full_name.to_string(),
                u64::from(repository.installation),
                repository.permission,
                repository.grant.is_some(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                format!("octo-cat/repo-{REPOSITORY}"),
                INSTALLATION,
                GitHubInstallationPermission::Granted,
                true
            ),
            (
                "octo-cat/repo-9999".to_owned(),
                8192,
                GitHubInstallationPermission::ApprovalRequired,
                false
            ),
        ]
    );
    let unapproved = exchange
        .repositories
        .get(1)
        .expect("No unapproved repository");
    assert_eq!(unapproved.installation_url.as_str(), installation_url(8192));
    let grant = exchange
        .repositories
        .first()
        .and_then(|repository| repository.grant.clone())
        .expect("No grant");
    let claims = server
        .token_key()
        .validate_github_grant(&grant, at(NOW))
        .expect("Invalid grant");
    assert_eq!((claims.sub, claims.project), (leader.uuid, project.uuid));
    assert_eq!(
        (u64::from(claims.installation), u64::from(claims.repository)),
        (INSTALLATION, REPOSITORY)
    );
    assert_eq!(claims.exp, NOW + TTL);

    let link: JsonLinkedRepository = ok(link_request(&server, &project, &leader, &grant)).await;

    assert_eq!(
        link.full_name.to_string(),
        format!("octo-cat/repo-{REPOSITORY}")
    );
    assert_eq!(
        (link.status, link.reason),
        (LinkedRepositoryStatus::Connected, None)
    );
    assert_eq!(link.user.uuid, leader.uuid);
    assert_eq!(link.linked, at(NOW));
    let [exchange, installations, first, second]: [Received; 4] = github
        .received()
        .await
        .try_into()
        .ok()
        .expect("Wrong request count");
    let form = exchange.form();
    assert_eq!(form.get("code").map(String::as_str), Some(CODE));
    assert_eq!(
        form.get("redirect_uri").map(String::as_str),
        Some(RETURN_URL)
    );
    for request in [&installations, &first, &second] {
        assert_eq!(
            request.header("authorization"),
            Some(format!("Bearer {USER_TOKEN}").as_str())
        );
    }
    assert_no_credential(&log, &[&state, grant.as_ref()]);
}

// Kills an exchange that lets one user spend another user's state. The fake GitHub answers
// nothing, so an exchange that reached it first would answer 400.
#[tokio::test]
async fn the_exchange_refuses_another_users_state() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    let maintainer = server
        .signup("Maintainer User", "maintainer@example.com")
        .await;
    grant_project_role(
        &server,
        &maintainer,
        project.slug.as_ref(),
        Role::Maintainer,
    );
    let state = state(&server, &leader, &project, at(NOW));

    let response = exchange_request(&server, &maintainer, &state)
        .send()
        .await
        .expect("Request failed");

    assert_refused(response, StatusCode::FORBIDDEN, "another user", &[&state]).await;
}

// Kills an exchange that accepts a state past its ten minutes, reads the system clock, or repeats
// the state in its refusal.
#[tokio::test]
async fn the_exchange_refuses_a_state_past_its_expiry() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    let fresh_state = state(&server, &leader, &project, at(NOW - TTL));
    let expired_state = state(&server, &leader, &project, at(NOW - TTL - 1));

    let expired = exchange_request(&server, &leader, &expired_state)
        .send()
        .await
        .expect("Request failed");
    // A fresh state reaches GitHub, which the fake leaves unanswered.
    let fresh = exchange_request(&server, &leader, &fresh_state)
        .send()
        .await
        .expect("Request failed");

    assert_refused(
        expired,
        StatusCode::BAD_REQUEST,
        "not valid",
        &[&expired_state],
    )
    .await;
    assert_refused(
        fresh,
        StatusCode::BAD_REQUEST,
        "GitHub repositories",
        &[&fresh_state],
    )
    .await;
}

// A Developer may edit a project but not manage it, so this kills `Edit` in place of `Manage`
// and any endpoint that skips the role. The fake GitHub answers nothing, so an exchange that
// reached it would answer 400.
#[tokio::test]
async fn a_caller_without_the_role_is_refused_at_each_endpoint() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    let developer = server
        .signup("Developer User", "developer@example.com")
        .await;
    grant_project_role(&server, &developer, project.slug.as_ref(), Role::Developer);
    let developer_grant = grant(
        &server,
        &developer,
        &project,
        at(NOW),
        INSTALLATION,
        REPOSITORY,
    );
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;

    for response in [
        request(&server, Method::GET, &github_path(&project, ""), &developer),
        request(
            &server,
            Method::DELETE,
            &github_path(&project, ""),
            &developer,
        ),
        request(
            &server,
            Method::GET,
            &github_path(&project, "/urls"),
            &developer,
        ),
        link_request(&server, &project, &developer, &developer_grant),
        exchange_request(
            &server,
            &developer,
            &state(&server, &developer, &project, at(NOW)),
        ),
    ] {
        let response = response.send().await.expect("Request failed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert!(stored(&server, &project).is_some());
}

// Kills a `PUT` that adds a link beside the old one or keeps the old one, and a `DELETE` that
// leaves the link or fails without one.
#[tokio::test]
async fn a_put_replaces_the_link_and_a_delete_removes_it() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    let maintainer = server
        .signup("Maintainer User", "maintainer@example.com")
        .await;
    grant_project_role(
        &server,
        &maintainer,
        project.slug.as_ref(),
        Role::Maintainer,
    );
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;

    let replaced = link(
        &server,
        &project,
        &maintainer,
        &grant(&server, &maintainer, &project, at(NOW), INSTALLATION, 4097),
    )
    .await;
    let got: JsonProjectGitHub = ok(request(
        &server,
        Method::GET,
        &github_path(&project, ""),
        &leader,
    ))
    .await;

    assert_eq!(replaced.full_name.to_string(), "octo-cat/repo-4097");
    assert_eq!(replaced.user.uuid, maintainer.uuid);
    let got = got.linked_repository.expect("No link");
    assert_eq!(got.full_name.to_string(), "octo-cat/repo-4097");
    assert_eq!(got.user.uuid, maintainer.uuid);
    assert_eq!(
        u64::from(stored(&server, &project).expect("No link").repository_id),
        4097
    );

    for _ in 0..2 {
        let response = request(&server, Method::DELETE, &github_path(&project, ""), &leader)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    let got: JsonProjectGitHub = ok(request(
        &server,
        Method::GET,
        &github_path(&project, ""),
        &leader,
    ))
    .await;
    assert!(got.linked_repository.is_none());
}

// Kills a `PUT` that skips the grant's project, its expiry, or its user, any of which lets a
// caller link a repository that their own GitHub token never showed they can push to, and a
// refusal that repeats the grant.
#[tokio::test]
async fn a_put_refuses_a_grant_for_another_project_user_or_time() {
    let Setup {
        server,
        leader,
        org,
        project,
        ..
    } = setup(Vec::new()).await;
    let other_project = server.create_project(&leader, &org, "Other Project").await;
    let maintainer = server
        .signup("Maintainer User", "maintainer@example.com")
        .await;
    grant_project_role(
        &server,
        &maintainer,
        project.slug.as_ref(),
        Role::Maintainer,
    );

    for (caller, grant, status, message) in [
        (
            &leader,
            grant(
                &server,
                &leader,
                &other_project,
                at(NOW),
                INSTALLATION,
                REPOSITORY,
            ),
            StatusCode::FORBIDDEN,
            "another project",
        ),
        (
            &maintainer,
            grant(
                &server,
                &leader,
                &project,
                at(NOW),
                INSTALLATION,
                REPOSITORY,
            ),
            StatusCode::FORBIDDEN,
            "another user",
        ),
        (
            &leader,
            grant(
                &server,
                &leader,
                &project,
                at(NOW - TTL - 1),
                INSTALLATION,
                REPOSITORY,
            ),
            StatusCode::BAD_REQUEST,
            "not valid",
        ),
        (
            &leader,
            state(&server, &leader, &project, at(NOW))
                .parse()
                .expect("Invalid state"),
            StatusCode::BAD_REQUEST,
            "not valid",
        ),
    ] {
        let response = link_request(&server, &project, caller, &grant)
            .send()
            .await
            .expect("Request failed");
        assert_refused(response, status, message, &[grant.as_ref()]).await;
    }
    assert!(stored(&server, &project).is_none());
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW - TTL),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;
}

// Kills a server without the app that serves any link endpoint, refuses without naming the token
// that still works, or writes a delivery it cannot verify.
#[tokio::test]
async fn a_server_without_the_app_refuses_links_and_ignores_deliveries() {
    let server = TestServer::new().await;
    server.signup("Admin User", "admin@example.com").await;
    let leader = server.signup("Leader User", "leader@example.com").await;
    let org = server.create_org(&leader, "GitHub Org").await;
    let project = server.create_project(&leader, &org, "GitHub Project").await;
    insert_link(&server, &project, &leader);

    for response in [
        request(&server, Method::GET, &github_path(&project, ""), &leader),
        request(&server, Method::DELETE, &github_path(&project, ""), &leader),
        request(
            &server,
            Method::GET,
            &github_path(&project, "/urls"),
            &leader,
        ),
        link_request(
            &server,
            &project,
            &leader,
            &grant(
                &server,
                &leader,
                &project,
                at(NOW),
                INSTALLATION,
                REPOSITORY,
            ),
        ),
        exchange_request(
            &server,
            &leader,
            &state(&server, &leader, &project, at(NOW)),
        ),
    ] {
        let response = response.send().await.expect("Request failed");
        assert_refused(response, StatusCode::NOT_FOUND, "--ci-callback-token", &[]).await;
    }
    let body = installation_event("deleted", INSTALLATION, None);
    let response = server
        .github_webhook("installation", Some(&github_signature(&body)), &body)
        .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let link = stored(&server, &project).expect("No link");
    assert_eq!(link.status, LinkedRepositoryStatus::Connected);
}

// Kills a webhook that writes before it checks the signature, accepts a delivery with no
// signature or one by any other secret, or answers a well-signed body it cannot parse with 401.
#[tokio::test]
async fn a_bad_delivery_is_refused_before_any_write() {
    let Setup {
        server,
        log,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;
    let body = installation_event("deleted", INSTALLATION, None);
    let forged = github_signature(&installation_event("deleted", 1, None));
    let unparsable = r#"{"action": "deleted"}"#;
    let signed = github_signature(unparsable);

    for (body, signature, status) in [
        (body.as_str(), None, StatusCode::UNAUTHORIZED),
        (
            body.as_str(),
            Some(forged.as_str()),
            StatusCode::UNAUTHORIZED,
        ),
        (body.as_str(), Some("sha256=00"), StatusCode::UNAUTHORIZED),
        (unparsable, Some(signed.as_str()), StatusCode::BAD_REQUEST),
    ] {
        let response = server.github_webhook("installation", signature, body).await;
        assert_eq!(response.status(), status, "{body} {signature:?}");
    }

    let link = stored(&server, &project).expect("No link");
    assert_eq!(link.status, LinkedRepositoryStatus::Connected);
    assert_no_credential(&log, &[]);
}

// Kills each transition: the wrong end state or reason, a transition on any other installation,
// a suspension that overwrites another reason, an uninstall that keeps one, an unsuspend that
// reconnects a link it did not disconnect, needs the repositories named, or ignores the ones it
// names, a reconnect that keeps the old installation or ignores which repositories it names, and
// a replay that changes anything.
#[tokio::test]
async fn each_delivery_moves_every_link_it_names() {
    let Setup {
        server,
        leader,
        org,
        project,
        ..
    } = setup(Vec::new()).await;
    // Two projects may link one repository, and every delivery moves both.
    let twin = server.create_project(&leader, &org, "Twin Project").await;
    for project in [&project, &twin] {
        link(
            &server,
            project,
            &leader,
            &grant(&server, &leader, project, at(NOW), INSTALLATION, REPOSITORY),
        )
        .await;
    }
    let disconnected = |reason| (LinkedRepositoryStatus::Disconnected, Some(reason));
    let connected = (LinkedRepositoryStatus::Connected, None);

    for (event, body, expected, installation) in [
        (
            "installation",
            installation_event("suspend", 1, None),
            connected,
            INSTALLATION,
        ),
        (
            "installation_repositories",
            repositories_event("removed", 1, &[], &[REPOSITORY]),
            connected,
            INSTALLATION,
        ),
        (
            "installation",
            installation_event("suspend", INSTALLATION, None),
            disconnected(LinkedRepositoryReason::Suspended),
            INSTALLATION,
        ),
        (
            "installation",
            installation_event("unsuspend", INSTALLATION, None),
            connected,
            INSTALLATION,
        ),
        (
            "installation_repositories",
            repositories_event("removed", INSTALLATION, &[], &[4097, REPOSITORY]),
            disconnected(LinkedRepositoryReason::Removed),
            INSTALLATION,
        ),
        (
            "installation",
            installation_event("suspend", INSTALLATION, None),
            disconnected(LinkedRepositoryReason::Removed),
            INSTALLATION,
        ),
        (
            "installation",
            installation_event("unsuspend", INSTALLATION, None),
            disconnected(LinkedRepositoryReason::Removed),
            INSTALLATION,
        ),
        (
            "installation",
            installation_event("deleted", INSTALLATION, None),
            disconnected(LinkedRepositoryReason::Uninstalled),
            INSTALLATION,
        ),
        (
            "installation_repositories",
            repositories_event("added", 8192, &[4097, REPOSITORY], &[]),
            connected,
            8192,
        ),
        (
            "installation",
            installation_event("suspend", 8192, None),
            disconnected(LinkedRepositoryReason::Suspended),
            8192,
        ),
        (
            "installation",
            installation_event("unsuspend", 1, None),
            disconnected(LinkedRepositoryReason::Suspended),
            8192,
        ),
        (
            "installation",
            installation_event("deleted", 8192, Some(&[REPOSITORY])),
            disconnected(LinkedRepositoryReason::Uninstalled),
            8192,
        ),
        (
            "installation",
            installation_event("unsuspend", 8192, None),
            disconnected(LinkedRepositoryReason::Uninstalled),
            8192,
        ),
        (
            "installation",
            installation_event("created", 9999, Some(&[4097])),
            disconnected(LinkedRepositoryReason::Uninstalled),
            8192,
        ),
        (
            "installation",
            installation_event("created", 9999, Some(&[4097, REPOSITORY])),
            connected,
            9999,
        ),
        (
            "installation",
            installation_event("new_permissions_accepted", 9999, None),
            connected,
            9999,
        ),
        (
            "ping",
            r#"{"zen": "Design for failure."}"#.to_owned(),
            connected,
            9999,
        ),
        (
            "installation_repositories",
            repositories_event("removed", 9999, &[], &[REPOSITORY]),
            disconnected(LinkedRepositoryReason::Removed),
            9999,
        ),
        (
            "installation",
            installation_event("unsuspend", 9999, Some(&[REPOSITORY])),
            connected,
            9999,
        ),
    ] {
        // GitHub may deliver twice, and the second must change nothing.
        for _ in 0..2 {
            let response = server
                .github_webhook(event, Some(&github_signature(&body)), &body)
                .await;
            assert_eq!(response.status(), StatusCode::ACCEPTED, "{body}");
            for project in [&project, &twin] {
                let link = stored(&server, project).expect("No link");
                assert_eq!((link.status, link.reason), expected, "{body}");
                assert_eq!(u64::from(link.installation_id), installation, "{body}");
            }
        }
    }
}

// A repository moves to another installation with an `added` there and a `removed` here, which
// GitHub may deliver in either order. Kills a reconnect that skips a link still connected.
#[tokio::test]
async fn a_repository_added_elsewhere_before_its_removal_stays_connected() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;

    for body in [
        repositories_event("added", 8192, &[REPOSITORY], &[]),
        repositories_event("removed", INSTALLATION, &[], &[REPOSITORY]),
    ] {
        let response = server
            .github_webhook(
                "installation_repositories",
                Some(&github_signature(&body)),
                &body,
            )
            .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "{body}");
    }

    let link = stored(&server, &project).expect("No link");
    assert_eq!(
        (link.status, link.reason),
        (LinkedRepositoryStatus::Connected, None)
    );
    assert_eq!(u64::from(link.installation_id), 8192);
}

// Two projects link two repositories in one installation, so this kills a removal that
// disconnects every link on the installation, and a lookup, a lookup for the mint, or an unlink
// that reaches another project's link.
#[tokio::test]
async fn each_project_keeps_its_own_link_in_a_shared_installation() {
    let Setup {
        server,
        leader,
        org,
        project,
        ..
    } = setup(Vec::new()).await;
    let other = server.create_project(&leader, &org, "Other Project").await;
    for (project, repository) in [(&project, REPOSITORY), (&other, 4097)] {
        link(
            &server,
            project,
            &leader,
            &grant(&server, &leader, project, at(NOW), INSTALLATION, repository),
        )
        .await;
    }
    let body = repositories_event("removed", INSTALLATION, &[], &[REPOSITORY]);

    let response = server
        .github_webhook(
            "installation_repositories",
            Some(&github_signature(&body)),
            &body,
        )
        .await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut conn = server.db_conn();
    for (project, repository, connected) in
        [(&project, REPOSITORY, None), (&other, 4097, Some(4097))]
    {
        let got: JsonProjectGitHub = ok(request(
            &server,
            Method::GET,
            &github_path(project, ""),
            &leader,
        ))
        .await;
        let project_id = stored(&server, project).expect("No link").project_id;
        let mint = QueryLinkedRepository::get_connected(&mut conn, project_id)
            .expect("Failed to read the link");

        assert_eq!(
            got.linked_repository
                .expect("No link")
                .full_name
                .to_string(),
            format!("octo-cat/repo-{repository}")
        );
        assert_eq!(mint.map(|link| u64::from(link.repository_id)), connected);
    }

    let response = request(&server, Method::DELETE, &github_path(&project, ""), &leader)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(stored(&server, &project).is_none());
    assert!(stored(&server, &other).is_some());
}

// The mint holds the link it read while GitHub answers, so this kills a write-back of the full name
// or a disconnect that lands on a link replaced since, on another installation or with another
// repository, a disconnect that overwrites another reason, a lookup that hands the mint a
// disconnected link, and a `GET` that hides a disconnected link or its reason.
#[tokio::test]
async fn the_mint_hooks_touch_only_the_link_they_read() {
    let Setup {
        server,
        leader,
        project,
        ..
    } = setup(Vec::new()).await;
    let mut conn = server.db_conn();
    let project_id = schema::project::table
        .filter(schema::project::uuid.eq(project.uuid))
        .select(schema::project::id)
        .first(&mut conn)
        .expect("No project");
    let renamed = "octo-cat/renamed".parse().expect("Invalid name");
    link(
        &server,
        &project,
        &leader,
        &grant(
            &server,
            &leader,
            &project,
            at(NOW),
            INSTALLATION,
            REPOSITORY,
        ),
    )
    .await;
    let read = QueryLinkedRepository::get_connected(&mut conn, project_id)
        .expect("Failed to read the link")
        .expect("No connected link");

    // Each replacement differs from the read in one key only.
    for (installation, repository) in [(8192, REPOSITORY), (INSTALLATION, 4097)] {
        link(
            &server,
            &project,
            &leader,
            &grant(
                &server,
                &leader,
                &project,
                at(NOW),
                installation,
                repository,
            ),
        )
        .await;

        assert!(
            !read
                .write_full_name(&mut conn, &renamed)
                .expect("Failed to write"),
            "{installation} {repository}"
        );
        assert!(
            !read
                .disconnect(&mut conn, LinkedRepositoryReason::Removed)
                .expect("Failed to write"),
            "{installation} {repository}"
        );
    }
    let replaced = stored(&server, &project).expect("No link");
    assert_eq!(replaced.full_name.to_string(), "octo-cat/repo-4097");
    assert_eq!(replaced.status, LinkedRepositoryStatus::Connected);

    assert!(
        replaced
            .write_full_name(&mut conn, &renamed)
            .expect("Failed to write")
    );
    assert!(
        replaced
            .disconnect(&mut conn, LinkedRepositoryReason::Uninstalled)
            .expect("Failed to write")
    );
    assert!(
        !replaced
            .disconnect(&mut conn, LinkedRepositoryReason::Removed)
            .expect("Failed to write")
    );
    let disconnected = stored(&server, &project).expect("No link");
    assert_eq!(disconnected.full_name, renamed);
    assert_eq!(
        (disconnected.status, disconnected.reason),
        (
            LinkedRepositoryStatus::Disconnected,
            Some(LinkedRepositoryReason::Uninstalled)
        )
    );
    assert!(
        QueryLinkedRepository::get_connected(&mut conn, project_id)
            .expect("Failed to read the link")
            .is_none()
    );
    let got: JsonProjectGitHub = ok(request(
        &server,
        Method::GET,
        &github_path(&project, ""),
        &leader,
    ))
    .await;
    let got = got.linked_repository.expect("No link");
    assert_eq!(
        (got.status, got.reason),
        (
            LinkedRepositoryStatus::Disconnected,
            Some(LinkedRepositoryReason::Uninstalled)
        )
    );
}

/// A request from `user`.
fn request(
    server: &TestServer,
    method: Method,
    path: &str,
    user: &TestUser,
) -> reqwest::RequestBuilder {
    server.client.request(method, server.api_url(path)).header(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(&user.token),
    )
}

fn link_request(
    server: &TestServer,
    project: &TestProject,
    user: &TestUser,
    grant: &Jwt,
) -> reqwest::RequestBuilder {
    request(server, Method::PUT, &github_path(project, ""), user)
        .json(&serde_json::json!({ "grant": grant }))
}

fn exchange_request(server: &TestServer, user: &TestUser, state: &str) -> reqwest::RequestBuilder {
    request(server, Method::POST, "/v0/github/exchange", user)
        .json(&serde_json::json!({ "code": CODE, "state": state }))
}

async fn link(
    server: &TestServer,
    project: &TestProject,
    user: &TestUser,
    grant: &Jwt,
) -> JsonLinkedRepository {
    ok(link_request(server, project, user, grant)).await
}

/// The body of a successful response.
async fn ok<T>(request: reqwest::RequestBuilder) -> T
where
    T: serde::de::DeserializeOwned,
{
    let response = request.send().await.expect("Request failed");
    let status = response.status();
    let text = response.text().await.expect("Failed to read the body");
    assert!(status.is_success(), "{status}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the body")
}

/// The refusal names why, and repeats none of the `credentials` the request carried.
async fn assert_refused(
    response: reqwest::Response,
    status: StatusCode,
    message: &str,
    credentials: &[&str],
) {
    let actual = response.status();
    let text = response.text().await.expect("Failed to read the body");
    assert_eq!(actual, status, "{text}");
    assert!(text.contains(message), "{message} not in {text}");
    for credential in [CODE, USER_TOKEN].iter().chain(credentials) {
        assert!(!text.contains(credential), "{credential} in {text}");
    }
}

fn github_path(project: &TestProject, rest: &str) -> String {
    let project: &str = project.slug.as_ref();
    format!("/v0/projects/{project}/github{rest}")
}

fn at(timestamp: i64) -> DateTime {
    DateTime::try_from(timestamp).expect("Invalid timestamp")
}

fn query(url: &url::Url, key: &str) -> String {
    url.query_pairs()
        .find_map(|(k, v)| (k == key).then(|| v.into_owned()))
        .expect("No such query pair")
}

/// A state that `user` started for `project` at `issued`.
fn state(server: &TestServer, user: &TestUser, project: &TestProject, issued: DateTime) -> String {
    server
        .token_key()
        .new_github_state(issued, 10 * 60, user.uuid, project.uuid)
        .expect("Failed to create a state")
        .to_string()
}

/// A grant that `user` got for `project` at `issued`, for a repository in an installation.
fn grant(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
    issued: DateTime,
    installation: u64,
    repository: u64,
) -> Jwt {
    let key = server.token_key();
    let state = key
        .validate_github_state(
            &state(server, user, project, issued)
                .parse()
                .expect("Invalid state"),
            issued,
        )
        .expect("Invalid state");
    key.new_github_grant(
        issued,
        10 * 60,
        &state,
        GitHubInstallationId::try_from(installation).expect("Invalid installation"),
        GitHubRepositoryId::try_from(repository).expect("Invalid repository"),
        format!("octo-cat/repo-{repository}")
            .parse()
            .expect("Invalid name"),
    )
    .expect("Failed to create a grant")
}

fn stored(server: &TestServer, project: &TestProject) -> Option<QueryLinkedRepository> {
    schema::linked_repository::table
        .inner_join(schema::project::table)
        .filter(schema::project::uuid.eq(project.uuid))
        .select(QueryLinkedRepository::as_select())
        .first(&mut server.db_conn())
        .optional()
        .expect("Failed to read the link")
}

/// A link written straight to the database, for a server that serves no link endpoint.
fn insert_link(server: &TestServer, project: &TestProject, user: &TestUser) {
    let mut conn = server.db_conn();
    let project_id = schema::project::table
        .filter(schema::project::uuid.eq(project.uuid))
        .select(schema::project::id)
        .first(&mut conn)
        .expect("No project");
    let user_id = schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::id)
        .first(&mut conn)
        .expect("No user");
    QueryLinkedRepository::connected(
        project_id,
        GitHubInstallationId::try_from(INSTALLATION).expect("Invalid installation"),
        GitHubRepositoryId::try_from(REPOSITORY).expect("Invalid repository"),
        format!("octo-cat/repo-{REPOSITORY}")
            .parse()
            .expect("Invalid name"),
        user_id,
        at(NOW),
    )
    .replace(&mut conn)
    .expect("Failed to write the link");
}

/// No line the server logged holds the webhook secret, the client secret, the user's GitHub
/// token, the code, or any of `tokens`.
fn assert_no_credential(log: &LogCapture, tokens: &[&str]) {
    let lines = log.lines();
    // The server logs every request it answers, so a missing line would hide nothing.
    assert!(
        lines.iter().any(|line| line.contains("/v0/")),
        "no request lines in {lines:?}"
    );
    for marker in [
        GITHUB_WEBHOOK_SECRET,
        GITHUB_CLIENT_SECRET,
        USER_TOKEN,
        CODE,
    ]
    .iter()
    .chain(tokens)
    {
        for line in &lines {
            assert!(!line.contains(marker), "{marker} in {line}");
        }
    }
}

fn exchanged() -> (u16, String) {
    (
        200,
        format!(r#"{{"access_token": "{USER_TOKEN}", "token_type": "bearer", "scope": ""}}"#),
    )
}

fn installation_url(id: u64) -> String {
    format!("https://github.com/settings/installations/{id}")
}

/// One page of installations, each with its contents access.
fn installations(installations: &[(u64, &str)]) -> (u16, String) {
    let installations: Vec<String> = installations
        .iter()
        .map(|(id, contents)| {
            format!(
                r#"{{"id": {id}, "html_url": "{}", "suspended_at": null, "permissions": {{"contents": "{contents}", "metadata": "read"}}}}"#,
                installation_url(*id)
            )
        })
        .collect();
    (
        200,
        format!(
            r#"{{"total_count": 0, "installations": [{}]}}"#,
            installations.join(", ")
        ),
    )
}

/// One page of repositories, each with whether the user can push to it.
fn repositories(repositories: &[(u64, bool)]) -> (u16, String) {
    let repositories: Vec<String> = repositories
        .iter()
        .map(|(id, push)| {
            format!(
                r#"{{"id": {id}, "name": "repo-{id}", "full_name": "octo-cat/repo-{id}", "owner": {{"login": "octo-cat"}}, "permissions": {{"push": {push}, "pull": true}}}}"#
            )
        })
        .collect();
    (
        200,
        format!(
            r#"{{"total_count": 0, "repositories": [{}]}}"#,
            repositories.join(", ")
        ),
    )
}

fn repository_list(ids: &[u64]) -> String {
    let repositories: Vec<String> = ids
        .iter()
        .map(|id| {
            format!(r#"{{"id": {id}, "name": "repo-{id}", "full_name": "octo-cat/repo-{id}"}}"#)
        })
        .collect();
    format!("[{}]", repositories.join(", "))
}

fn installation_event(action: &str, installation: u64, repositories: Option<&[u64]>) -> String {
    let repositories = repositories
        .map(|ids| format!(r#""repositories": {}, "#, repository_list(ids)))
        .unwrap_or_default();
    format!(
        r#"{{"action": "{action}", "installation": {{"id": {installation}}}, {repositories}"sender": {{"login": "octo-cat"}}}}"#
    )
}

fn repositories_event(action: &str, installation: u64, added: &[u64], removed: &[u64]) -> String {
    format!(
        r#"{{"action": "{action}", "installation": {{"id": {installation}}}, "repository_selection": "selected", "repositories_added": {}, "repositories_removed": {}, "requester": null, "sender": {{"login": "octo-cat"}}}}"#,
        repository_list(added),
        repository_list(removed)
    )
}
