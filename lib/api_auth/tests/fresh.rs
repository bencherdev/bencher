#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the fresh sign-in: the window that guards the sensitive actions, and
//! proving a session again by an email to the reader's address or by a provider pinned to them.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use bencher_api_tests::{
    LogCapture, TestOrg, TestProject, TestServer, TestServerOptions, TestUser,
    github::{GitHubServer, Unscripted},
    helpers::{github_client, google_client, seed_token},
    session_cookie,
};
use bencher_json::{
    Clock, DateTime, JsonFreshSignIn, JsonOAuthUrl, JsonProved, JsonProviderSignIn, JsonSignInAs,
    JsonSignInEmail, PlanLevel, SessionToken, SessionTokenHash, SignInCode, SignInProvider,
    TRIES_LEFT, UserKey,
};
use bencher_schema::{
    context::RateLimiting,
    model::user::session::cookie::{PendingCookie, ProviderCookie, SessionCookie},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{HeaderMap, Method, StatusCode, header};
use serde_json::{Value, json};
use url::Url;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const WINDOW: i64 = 10 * MINUTE;
const CODE: &str = "code-marker";
const ADA: &str = "ada@example.com";

// Each guarded action passes a session proved 1 second inside the window, and is refused with
// the reason the console reads at the window's edge, changing nothing.
// Kills a guard missing from any of the five actions, a window of another length, and a window
// that counts its last second.
#[tokio::test]
async fn guarded_actions_need_a_sign_in_within_ten_minutes() {
    let Harness { server, clock, .. } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let org = server.create_org(&user, "Ada Org").await;
    let project = server.create_project(&user, &org, "Ada Project").await;
    let proved = clock.now();
    let session = server.session_at(&user, proved);
    let browser = Browser::signed_in(&session);

    for action in Action::ALL {
        clock.set(proved, WINDOW);
        let resp = action.send(&server, &user, &org, &project, &browser).await;
        assert_fresh_sign_in_needed(resp, action).await;
        action.assert_undone(&server, &user, &org, &project);

        clock.set(proved, WINDOW - 1);
        let resp = action.send(&server, &user, &org, &project, &browser).await;
        assert!(resp.status().is_success(), "{action:?}: {}", resp.status());
        action.assert_done(&server, &user, &org, &project);
    }
}

// A user key and a deprecated API token each do every guarded action they could do before, with
// no session at all, and are refused each one they could not, as before and not for want of a
// fresh sign-in.
// Kills a guard that refuses a user key or a deprecated API token.
#[tokio::test]
async fn bearer_credentials_need_no_fresh_sign_in() {
    let Harness { server, clock, .. } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let key = server.create_user_key(&user, "ada-key").await;
    let token = seed_token(&server, &user, "ada-token").token;
    clock.advance(HOUR);

    for (credential, bearer) in [
        ("user key", Browser::bearer(&key)),
        ("API token", Browser::jwt(token.as_ref())),
    ] {
        let org = server
            .create_org(&user, &format!("Ada Org {credential}"))
            .await;
        let project = server
            .create_project(&user, &org, &format!("Ada Project {credential}"))
            .await;
        for action in Action::ALL {
            let resp = action.send(&server, &user, &org, &project, &bearer).await;
            if action.takes_a_bearer() {
                assert!(
                    resp.status().is_success(),
                    "{credential} {action:?}: {}",
                    resp.status()
                );
                action.assert_done(&server, &user, &org, &project);
            } else {
                assert_eq!(
                    resp.status(),
                    StatusCode::FORBIDDEN,
                    "{credential} {action:?}"
                );
                let (code, message) = error(resp).await;
                assert_eq!(code, "", "{credential} {action:?}");
                assert!(message.starts_with("Only a user session"), "{message}");
                action.assert_undone(&server, &user, &org, &project);
            }
        }
    }
}

// A session JWT from the old sign-in holds no proof time, so it does no guarded action, however
// new it is.
// Kills a guard that lets a session without a session row pass.
#[tokio::test]
async fn old_session_jwt_does_not_pass() {
    let Harness { server, .. } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let org = server.create_org(&user, "Ada Org").await;
    let project = server.create_project(&user, &org, "Ada Project").await;
    let jwt = Browser::jwt(&user.token);

    for action in Action::ALL {
        let resp = action.send(&server, &user, &org, &project, &jwt).await;
        assert_fresh_sign_in_needed(resp, action).await;
        action.assert_undone(&server, &user, &org, &project);
    }
}

// A code proves the session that asked for it again and makes nothing: no session, no account,
// no cookie but the cleared pending one, and the email reads as a fresh sign-in's.
// Kills a proof that signs in anew, moves no proof time, or keeps its pending sign-in.
#[tokio::test]
async fn email_code_moves_proved_and_nothing_else() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let org = server.create_org(&user, "Ada Org").await;
    let project = server.create_project(&user, &org, "Ada Project").await;
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);

    browser.start(&server).await;
    let email = log.email("Sign in again");
    assert!(email.contains("Subject: Your Bencher code is "), "{email}");
    assert!(email.contains("Confirm it's you"), "{email}");
    assert!(email.contains("/auth/confirm#"), "{email}");
    let (code, _link) = log.sign_in();
    let before = Snapshot::of(&server);
    clock.advance(MINUTE);

    let resp = browser.prove(&server, &code).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(cookies(resp.headers()), [pending_clear()]);
    let JsonProved { proved } = resp.json().await.expect("Invalid answer");
    assert_eq!(proved, clock.now());
    assert_eq!(
        Snapshot::of(&server),
        before.proved(browser.session(), clock.now())
    );
    assert_eq!(pending_sign_ins(&server), 0);

    let resp = Action::NewUserKey
        .send(&server, &user, &org, &project, &browser)
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// A link proves the session again at once in the browser that asked for it, and makes nothing.
// Kills a link that signs in anew, or proves a session other than the one that asked.
#[tokio::test]
async fn email_link_moves_proved_and_nothing_else() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let other = server.session(&user);
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);

    browser.start(&server).await;
    let (_code, link) = log.sign_in();
    let before = Snapshot::of(&server);
    clock.advance(MINUTE);

    let resp = browser.link(&server, &link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(cookies(resp.headers()), [pending_clear()]);
    let json: JsonSignInAs = resp.json().await.expect("Invalid answer");
    assert_eq!(json.email.as_ref(), ADA);
    assert!(json.signed_in.is_none());
    assert_eq!(json.proved, Some(clock.now()));
    assert_eq!(
        Snapshot::of(&server),
        before.proved(browser.session(), clock.now())
    );
    assert_ne!(proved_of(&server, &other), clock.now());

    let resp = browser.link(&server, &link, false).await;
    assert_ended(resp).await;
}

// A link opened in any browser but the one that asked, confirmed or not, and signed in as the
// same user or not, proves nothing, makes nothing, and leaves the sign-in for the browser that
// asked.
// Kills a fresh link that proves elsewhere, or asks "Sign in as" and signs that browser in.
#[tokio::test]
async fn email_link_proves_only_in_the_browser_that_asked() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    let elsewhere = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);
    browser.start(&server).await;
    let (_code, link) = log.sign_in();
    let before = Snapshot::of(&server);

    for other in [&Browser::default(), &elsewhere] {
        for confirm in [false, true] {
            let resp = other.link(&server, &link, confirm).await;
            assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{confirm}");
            assert_eq!(cookies(resp.headers()), Vec::<String>::new());
            assert_eq!(error(resp).await.0, "asked_elsewhere");
        }
    }
    assert_eq!(Snapshot::of(&server), before);
    assert_eq!(pending_sign_ins(&server), 1);

    let resp = browser.link(&server, &link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(proved_of(&server, browser.session()), clock.now());
}

// The code works only in the session and the browser that asked, takes 5 tries, and works once.
// Kills a code taken by another session of the same user or by the sign-in's own code endpoint,
// a try cap of another size, and a code that works twice.
#[tokio::test]
async fn email_code_works_once_in_the_session_that_asked_with_five_tries() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);
    browser.start(&server).await;
    let (code, _link) = log.sign_in();
    let before = Snapshot::of(&server);

    // The same browser, signed in to another session of the same user.
    let other = Browser {
        session: Some(server.session(&user)),
        pending: browser.pending.clone(),
        bearer: None,
    };
    assert_ended(other.prove(&server, &code).await).await;
    // The sign-in's own code endpoint, which would make a new session.
    let resp = send(
        &server,
        Method::POST,
        "/v0/auth/code",
        &browser,
        Some(&json!({"code": code.as_ref()})),
    )
    .await;
    assert_ended(resp).await;
    let no_cookie = Browser::signed_in(browser.session());
    assert_ended(no_cookie.prove(&server, &code).await).await;
    assert_eq!(
        Snapshot::of(&server).sessions.len(),
        before.sessions.len() + 1
    );

    let wrong = wrong_code(&code);
    for tries_left in (1..=4).rev() {
        let resp = browser.prove(&server, &wrong).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            header_value(resp.headers(), TRIES_LEFT),
            Some(tries_left.to_string().as_str())
        );
        assert_eq!(error(resp).await.0, "wrong_code");
    }
    let resp = browser.prove(&server, &wrong).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(cookies(resp.headers()), [pending_clear()]);
    assert_eq!(error(resp).await.0, "no_tries_left");
    assert_ended(browser.prove(&server, &code).await).await;
    assert_ne!(proved_of(&server, browser.session()), clock.now());

    browser.start(&server).await;
    let (code, _link) = log.sign_in();
    assert_eq!(browser.prove(&server, &code).await.status(), StatusCode::OK);
    assert_ended(browser.prove(&server, &code).await).await;
}

// A door sign-in's code, wrong or right, posted to the fresh endpoint with a stale session cookie,
// counts no try, proves nothing, and ends nothing, so a stolen stale cookie cannot turn an emailed
// sign-in into a fresh proof; the sign-in still works at the door, with every try.
// Kills a fresh proof that takes a door sign-in's row.
#[tokio::test]
async fn door_code_never_proves_a_session() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let stolen = server.session(&user);
    clock.advance(HOUR);
    let mut attacker = Browser::signed_in(&stolen);
    attacker.start_sign_in(&server, ADA).await;
    let (code, _link) = log.sign_in();

    assert_ended(attacker.prove(&server, &wrong_code(&code)).await).await;
    // The stolen cookie's use marks its session used, and nothing else.
    let before = Snapshot::of(&server);
    assert_ended(attacker.prove(&server, &code).await).await;
    assert_eq!(Snapshot::of(&server), before);
    assert_ne!(proved_of(&server, &stolen), clock.now());

    let door = Browser {
        session: None,
        ..attacker
    };
    let resp = send(
        &server,
        Method::POST,
        "/v0/auth/code",
        &door,
        Some(&json!({"code": wrong_code(&code).as_ref()})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(header_value(resp.headers(), TRIES_LEFT), Some("4"));
    let resp = send(
        &server,
        Method::POST,
        "/v0/auth/code",
        &door,
        Some(&json!({"code": code.as_ref()})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// A fresh sign-in's code ends with the sign-in, 30 minutes after it starts.
// Kills a fresh code that outlives its sign-in.
#[tokio::test]
async fn fresh_code_ends_with_its_sign_in() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);
    browser.start(&server).await;
    let (code, _link) = log.sign_in();

    clock.advance(30 * MINUTE);
    assert_ended(browser.prove(&server, &code).await).await;
    assert_ne!(proved_of(&server, browser.session()), clock.now());
}

// A server admin's change to another user's email takes effect at once, so it needs the admin's
// fresh sign-in, and a stale one leaves the email as it was.
// Kills a guard on one's own email alone.
#[tokio::test]
async fn admin_changes_another_users_email_only_when_fresh() {
    let Harness {
        server,
        clock,
        admin,
        ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let proved = clock.now();
    let browser = Browser::signed_in(&server.session_at(&admin, proved));
    let path = format!("/v0/users/{}", user.uuid);
    let body = json!({"email": "ada-new@example.com"});

    clock.set(proved, WINDOW);
    let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error(resp).await.0, "fresh_sign_in");
    assert_eq!(email_of(&server, &user), ADA);

    clock.set(proved, WINDOW - 1);
    let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(email_of(&server, &user), "ada-new@example.com");
}

// Making a user a server admin, or not, gives or takes the power to do every guarded action, so
// it needs a fresh sign-in, and a stale one changes nothing.
// Kills an admin toggle without the guard.
#[tokio::test]
async fn admin_toggle_needs_a_fresh_sign_in() {
    let Harness {
        server,
        clock,
        admin,
        ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let proved = clock.now();
    let browser = Browser::signed_in(&server.session_at(&admin, proved));
    let path = format!("/v0/users/{}", user.uuid);

    for is_admin in [true, false] {
        let body = json!({"admin": is_admin});
        clock.set(proved, WINDOW);
        let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{is_admin}");
        assert_eq!(error(resp).await.0, "fresh_sign_in", "{is_admin}");
        assert_eq!(is_admin_of(&server, &user), !is_admin);

        clock.set(proved, WINDOW - 1);
        let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{is_admin}");
        assert_eq!(is_admin_of(&server, &user), is_admin);
    }
}

// Locking a user ends every session of theirs, and unlocking lets them in again, so each needs a
// fresh sign-in, and a stale one changes nothing.
// Kills a lock or an unlock without the guard.
#[tokio::test]
async fn lock_and_unlock_need_a_fresh_sign_in() {
    let Harness {
        server,
        clock,
        admin,
        ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let proved = clock.now();
    let browser = Browser::signed_in(&server.session_at(&admin, proved));
    let path = format!("/v0/users/{}", user.uuid);

    for locked in [true, false] {
        let body = json!({"locked": locked});
        clock.set(proved, WINDOW);
        let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{locked}");
        assert_eq!(error(resp).await.0, "fresh_sign_in", "{locked}");
        assert_eq!(is_locked_of(&server, &user), !locked);

        clock.set(proved, WINDOW - 1);
        let resp = send(&server, Method::PATCH, &path, &browser, Some(&body)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{locked}");
        assert_eq!(is_locked_of(&server, &user), locked);
    }
}

// A fresh sign-in's wrong codes spend the same budget as a sign-in's for the address, and while
// codes are paused the right code proves nothing, but the link still does.
// Kills a fresh code that skips the budget, or spends none of it.
#[tokio::test]
async fn email_code_counts_against_the_address_budget() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);

    // Five wrong codes by the door's sign-in, then five by fresh sign-ins.
    let mut door = Browser::default();
    door.start_sign_in(&server, ADA).await;
    let (code, _link) = log.sign_in();
    for _ in 0..5 {
        let resp = send(
            &server,
            Method::POST,
            "/v0/auth/code",
            &door,
            Some(&json!({"code": wrong_code(&code).as_ref()})),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
    browser.start(&server).await;
    let (code, _link) = log.sign_in();
    for _ in 0..5 {
        let resp = browser.prove(&server, &wrong_code(&code)).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    browser.start(&server).await;
    let (code, link) = log.sign_in();
    let resp = browser.prove(&server, &code).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(header_value(resp.headers(), header::RETRY_AFTER.as_str()).is_some());
    assert_eq!(error(resp).await.0, "codes_paused");
    assert_ne!(proved_of(&server, browser.session()), clock.now());

    let resp = browser.link(&server, &link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(proved_of(&server, browser.session()), clock.now());
}

// Resend opens after a minute and ends the code and link it replaces, and its own prove the same
// session again.
// Kills a Resend that drops the session it proves, or that keeps the old code alive.
#[tokio::test]
async fn resend_waits_a_minute_and_proves_the_same_session() {
    let Harness {
        server, clock, log, ..
    } = Harness::new().await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    clock.advance(HOUR);
    browser.start(&server).await;
    let (old_code, old_link) = log.sign_in();
    let old = browser.clone();

    clock.advance(MINUTE - 1);
    let resp = browser.resend(&server).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error(resp).await.0, "resend_waiting");
    clock.advance(1);
    let resp = browser.resend(&server).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    browser.pending = Some(pending_cookie(resp.headers()).expect("No pending cookie"));
    let (code, _link) = log.sign_in();

    assert_ended(old.prove(&server, &old_code).await).await;
    assert_ended(browser.link(&server, &old_link, false).await).await;
    assert_eq!(browser.prove(&server, &code).await.status(), StatusCode::OK);
    assert_eq!(proved_of(&server, browser.session()), clock.now());
}

// Signing out ends the session's fresh sign-ins, by email and by a provider.
// Kills a fresh sign-in that outlives the session it proves.
#[tokio::test]
async fn sign_out_ends_the_sessions_fresh_sign_ins() {
    let github = GitHubServer::start(Vec::new()).await;
    let Harness { server, log, .. } = Harness::with_providers(&github.base()).await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    browser.start(&server).await;
    let (_code, link) = log.sign_in();
    let started = browser.start_provider(&server, "github").await;

    let resp = send(&server, Method::DELETE, "/v0/auth/session", &browser, None).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(pending_sign_ins(&server), 0);
    assert_eq!(provider_states(&server), 0);
    assert_ended(browser.link(&server, &link, false).await).await;
    let resp = browser.finish(&server, "github", &started).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error(resp).await.0, "provider_refused");
}

// The address's cap holds for a fresh sign-in too: past it, no email goes out and the wait comes
// back.
// Kills a fresh sign-in that emails past the address's cap.
#[tokio::test]
async fn fresh_email_at_the_cap_answers_its_wait() {
    let Harness { server, log, .. } = Harness::with_options(TestServerOptions {
        rate_limiting: Some(RateLimiting::default()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server.signup("Ada", ADA).await;
    let mut browser = Browser::signed_in(&server.session(&user));
    // Two emails a minute to an address, by default.
    browser.start(&server).await;
    browser.start(&server).await;
    let emails = log.sign_in_emails().len();

    let resp = send(
        &server,
        Method::POST,
        "/v0/auth/fresh/email",
        &browser,
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(pending_cookie(resp.headers()), None);
    let json: JsonSignInEmail = resp.json().await.expect("Invalid answer");
    assert!(json.wait.is_some_and(|wait| wait > 0));
    assert_eq!(log.sign_in_emails().len(), emails);
}

// A provider account pinned to the session's user proves the session again and makes nothing; an
// account pinned to another user, or to no one, proves nothing and makes nothing.
// Kills a fresh sign-in that proves by any account, signs in, links, pins, or emails.
#[tokio::test]
async fn provider_proves_only_by_the_account_pinned_to_the_reader() {
    let github = GitHubServer::start(
        [
            github_account(42, "ada-gh", ADA),
            github_account(43, "grace-gh", "grace@example.com"),
            github_account(44, "nobody-gh", ADA),
            github_account(45, "new-gh", "new@example.com"),
        ]
        .concat(),
    )
    .await;
    let Harness {
        server, clock, log, ..
    } = Harness::with_providers(&github.base()).await;
    let ada = server.signup("Ada", ADA).await;
    let grace = server.signup("Grace", "grace@example.com").await;
    pin(&server, &ada, SignInProvider::Github, "42", "ada-gh");
    pin(&server, &grace, SignInProvider::Github, "43", "grace-gh");
    let browser = Browser::signed_in(&server.session(&ada));
    clock.advance(HOUR);
    let emails = log.sign_in_emails().len();

    let started = browser.start_provider(&server, "github").await;
    let before = Snapshot::of(&server);
    clock.advance(MINUTE);
    let resp = browser.finish(&server, "github", &started).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(cookies(resp.headers()), [provider_clear()]);
    let json: JsonProviderSignIn = resp.json().await.expect("Invalid answer");
    assert!(json.signed_in.is_none() && json.inbox.is_none());
    assert_eq!(json.proved, Some(clock.now()));
    let proved = clock.now();
    assert_eq!(
        Snapshot::of(&server),
        before.proved(browser.session(), proved)
    );

    for account in ["grace's", "pinned to no one, with ada's email", "a new one"] {
        let before = Snapshot::of(&server);
        let started = browser.start_provider(&server, "github").await;
        clock.advance(MINUTE);
        let resp = browser.finish(&server, "github", &started).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{account}");
        assert_eq!(cookies(resp.headers()), [provider_clear()], "{account}");
        assert_eq!(error(resp).await.0, "provider_refused", "{account}");
        assert_eq!(Snapshot::of(&server), before, "{account}");
        assert_eq!(proved_of(&server, browser.session()), proved, "{account}");
    }
    assert_eq!(pending_sign_ins(&server), 0);
    assert_eq!(log.sign_in_emails().len(), emails);
    github.received().await;
}

// A locked user's pinned account proves nothing, though a lock that ended the session already
// ends its fresh sign-in.
// Kills a fresh sign-in by provider that skips the lock.
#[tokio::test]
async fn provider_refuses_a_locked_reader() {
    let github = GitHubServer::start(github_account(42, "ada-gh", ADA)).await;
    let Harness { server, clock, .. } = Harness::with_providers(&github.base()).await;
    let ada = server.signup("Ada", ADA).await;
    pin(&server, &ada, SignInProvider::Github, "42", "ada-gh");
    let browser = Browser::signed_in(&server.session(&ada));
    clock.advance(HOUR);
    let started = browser.start_provider(&server, "github").await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(ada.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");

    let resp = browser.finish(&server, "github", &started).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error(resp).await.0, "provider_refused");
    assert_ne!(proved_of(&server, browser.session()), clock.now());
    github.received().await;
}

// Every fresh sign-in endpoint needs the console's origin and a browser session: a request from
// another site is refused before anything, and so is one with no credential, a user key, or a
// session JWT from the old sign-in.
// Kills a fresh endpoint without the same-origin check, or one that takes a bearer credential.
#[tokio::test]
async fn fresh_endpoints_need_the_console_origin_and_a_session() {
    let github = Unscripted::bind().await;
    let Harness { server, log, .. } = Harness::with_providers(&github.base()).await;
    let user = server.signup("Ada", ADA).await;
    let key = server.create_user_key(&user, "ada-key").await;
    let browser = Browser::signed_in(&server.session(&user));
    let emails = log.sign_in_emails().len();
    let posts = [
        "/v0/auth/fresh/email",
        "/v0/auth/fresh/email/resend",
        "/v0/auth/fresh/code",
        "/v0/auth/fresh/providers/github/start",
    ];
    let body = json!({"code": "000000"});

    for path in posts {
        let resp = server
            .client
            .post(server.api_url(path))
            .header(header::COOKIE, session_cookie(browser.session()))
            .header("Sec-Fetch-Site", "cross-site")
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
        assert_eq!(error(resp).await.0, "cross_origin", "{path}");

        for (bearer, status) in [
            (Browser::default(), StatusCode::UNAUTHORIZED),
            (Browser::bearer(&key), StatusCode::FORBIDDEN),
            (Browser::jwt(&user.token), StatusCode::FORBIDDEN),
        ] {
            let resp = send(&server, Method::POST, path, &bearer, Some(&body)).await;
            assert_eq!(resp.status(), status, "{path}");
        }
    }
    for (bearer, status) in [
        (Browser::default(), StatusCode::UNAUTHORIZED),
        (Browser::bearer(&key), StatusCode::FORBIDDEN),
        (Browser::jwt(&user.token), StatusCode::FORBIDDEN),
    ] {
        let resp = send(&server, Method::GET, "/v0/auth/fresh", &bearer, None).await;
        assert_eq!(resp.status(), status);
    }
    assert_eq!(pending_sign_ins(&server), 0);
    assert_eq!(provider_states(&server), 0);
    assert_eq!(log.sign_in_emails().len(), emails);
}

// The ways to sign in again list each provider pinned to the reader that the server signs in
// with, and no other.
// Kills a list of every provider the server has, of every pin whatever the server has, or of
// pins on a server without a license.
#[tokio::test]
async fn fresh_lists_the_providers_pinned_to_the_reader() {
    let github = Unscripted::bind().await;
    let Harness { server, .. } = Harness::with_providers(&github.base()).await;
    let ada = server.signup("Ada", ADA).await;
    let grace = server.signup("Grace", "grace@example.com").await;
    pin(&server, &ada, SignInProvider::Github, "42", "ada-gh");
    pin(&server, &ada, SignInProvider::Google, "g-42", ADA);
    assert_eq!(
        providers(&server, &ada).await,
        [SignInProvider::Github, SignInProvider::Google]
    );
    assert_eq!(providers(&server, &grace).await, []);

    // Google is not configured here, and nothing is licensed.
    let unlicensed = TestServer::new_with_options(TestServerOptions {
        github_client: Some(github_client().with_base_url(github.base())),
        ..TestServerOptions::default()
    })
    .await;
    let ada = unlicensed.signup("Ada", ADA).await;
    pin(&unlicensed, &ada, SignInProvider::Github, "42", "ada-gh");
    assert_eq!(providers(&unlicensed, &ada).await, []);
    let Harness {
        server: github_only,
        ..
    } = Harness::with_options(TestServerOptions {
        github_client: Some(github_client().with_base_url(github.base())),
        ..TestServerOptions::default()
    })
    .await;
    let ada = github_only.signup("Ada", ADA).await;
    pin(&github_only, &ada, SignInProvider::Github, "42", "ada-gh");
    pin(&github_only, &ada, SignInProvider::Google, "g-42", ADA);
    assert_eq!(
        providers(&github_only, &ada).await,
        [SignInProvider::Github]
    );
}

/// The five actions that need a fresh sign-in.
#[derive(Debug, Clone, Copy)]
enum Action {
    NewUserKey,
    NewProjectKey,
    EmailChange,
    DeleteProject,
    DeleteOrganization,
}

impl Action {
    /// In an order where each leaves the next one to do: the project goes before its organization.
    const ALL: [Self; 5] = [
        Self::NewUserKey,
        Self::NewProjectKey,
        Self::EmailChange,
        Self::DeleteProject,
        Self::DeleteOrganization,
    ];

    async fn send(
        self,
        server: &TestServer,
        user: &TestUser,
        org: &TestOrg,
        project: &TestProject,
        browser: &Browser,
    ) -> reqwest::Response {
        let (method, path, body) = match self {
            Self::NewUserKey => (
                Method::POST,
                format!("/v0/users/{}/keys", user.slug),
                Some(json!({"name": "fresh-key"})),
            ),
            Self::NewProjectKey => (
                Method::POST,
                format!("/v0/projects/{}/keys", project.slug),
                Some(json!({"name": "fresh-key"})),
            ),
            Self::EmailChange => (
                Method::PATCH,
                format!("/v0/users/{}", user.uuid),
                Some(json!({"email": "ada-new@example.com"})),
            ),
            Self::DeleteProject => (
                Method::DELETE,
                format!("/v0/projects/{}", project.slug),
                None,
            ),
            Self::DeleteOrganization => (
                Method::DELETE,
                format!("/v0/organizations/{}", org.slug),
                None,
            ),
        };
        send(server, method, &path, browser, body.as_ref()).await
    }

    /// Whether a user key could do it before a fresh sign-in was asked for.
    fn takes_a_bearer(self) -> bool {
        matches!(self, Self::DeleteProject | Self::DeleteOrganization)
    }

    fn assert_undone(
        self,
        server: &TestServer,
        user: &TestUser,
        org: &TestOrg,
        project: &TestProject,
    ) {
        assert!(!self.is_done(server, user, org, project), "{self:?}");
    }

    fn assert_done(
        self,
        server: &TestServer,
        user: &TestUser,
        org: &TestOrg,
        project: &TestProject,
    ) {
        assert!(self.is_done(server, user, org, project), "{self:?}");
    }

    fn is_done(
        self,
        server: &TestServer,
        user: &TestUser,
        org: &TestOrg,
        project: &TestProject,
    ) -> bool {
        let mut conn = server.db_conn();
        match self {
            Self::NewUserKey => {
                schema::user_key::table
                    .filter(schema::user_key::name.eq("fresh-key"))
                    .count()
                    .get_result::<i64>(&mut conn)
                    .expect("Failed to count user keys")
                    > 0
            },
            Self::NewProjectKey => {
                schema::project_key::table
                    .filter(schema::project_key::name.eq("fresh-key"))
                    .count()
                    .get_result::<i64>(&mut conn)
                    .expect("Failed to count project keys")
                    > 0
            },
            // One's own new email waits on its confirm link, but the asking writes the change.
            Self::EmailChange => {
                schema::email_change::table
                    .inner_join(schema::user::table)
                    .filter(schema::user::uuid.eq(user.uuid))
                    .count()
                    .get_result::<i64>(&mut conn)
                    .expect("Failed to count email changes")
                    > 0
            },
            Self::DeleteProject => schema::project::table
                .filter(schema::project::uuid.eq(project.uuid))
                .select(schema::project::deleted)
                .first::<Option<i64>>(&mut conn)
                .expect("Failed to read the project")
                .is_some(),
            Self::DeleteOrganization => schema::organization::table
                .filter(schema::organization::uuid.eq(org.uuid))
                .select(schema::organization::deleted)
                .first::<Option<i64>>(&mut conn)
                .expect("Failed to read the organization")
                .is_some(),
        }
    }
}

/// A browser, or a script: the session cookie it presents, the pending sign-in cookie it holds
/// once it has one, and the bearer credential it sends, if any.
#[derive(Clone, Default)]
struct Browser {
    session: Option<SessionToken>,
    pending: Option<String>,
    bearer: Option<String>,
}

impl Browser {
    fn signed_in(session: &SessionToken) -> Self {
        Self {
            session: Some(session.clone()),
            ..Self::default()
        }
    }

    fn bearer(key: &UserKey) -> Self {
        Self {
            bearer: Some(key.as_ref().to_owned()),
            ..Self::default()
        }
    }

    fn jwt(token: &str) -> Self {
        Self {
            bearer: Some(token.to_owned()),
            ..Self::default()
        }
    }

    fn session(&self) -> &SessionToken {
        self.session.as_ref().expect("Not signed in")
    }

    /// Ask for a fresh sign-in by email, keeping its pending cookie.
    async fn start(&mut self, server: &TestServer) {
        let resp = send(server, Method::POST, "/v0/auth/fresh/email", self, None).await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        assert_eq!(session_cookie_set(resp.headers()), None);
        self.pending = Some(pending_cookie(resp.headers()).expect("No pending cookie"));
        let json: JsonSignInEmail = resp.json().await.expect("Invalid answer");
        assert_eq!(json.email.as_ref(), ADA);
    }

    /// Start the door's sign-in by email for `email`, keeping its pending cookie.
    async fn start_sign_in(&mut self, server: &TestServer, email: &str) {
        let body = json!({"email": email});
        let resp = send(server, Method::POST, "/v0/auth/email", self, Some(&body)).await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        self.pending = Some(pending_cookie(resp.headers()).expect("No pending cookie"));
    }

    async fn prove(&self, server: &TestServer, code: &SignInCode) -> reqwest::Response {
        let body = json!({"code": code.as_ref()});
        send(
            server,
            Method::POST,
            "/v0/auth/fresh/code",
            self,
            Some(&body),
        )
        .await
    }

    async fn resend(&self, server: &TestServer) -> reqwest::Response {
        send(
            server,
            Method::POST,
            "/v0/auth/fresh/email/resend",
            self,
            None,
        )
        .await
    }

    async fn link(
        &self,
        server: &TestServer,
        link: &bencher_json::LinkSecret,
        confirm: bool,
    ) -> reqwest::Response {
        let body = json!({"secret": link.as_ref(), "confirm": confirm});
        send(server, Method::POST, "/v0/auth/link", self, Some(&body)).await
    }

    /// Ask for a fresh sign-in with `provider`, returning its state and its binding cookie.
    async fn start_provider(&self, server: &TestServer, provider: &str) -> Started {
        let path = format!("/v0/auth/fresh/providers/{provider}/start");
        let resp = send(server, Method::POST, &path, self, None).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let binding = cookie_value(resp.headers(), ProviderCookie::new(&console_url()).name())
            .expect("No provider cookie");
        let JsonOAuthUrl { url } = resp.json().await.expect("Invalid answer");
        let state = url
            .query_pairs()
            .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
            .expect("No state");
        Started { state, binding }
    }

    /// Come back from `provider` to finish the fresh sign-in `started`.
    async fn finish(
        &self,
        server: &TestServer,
        provider: &str,
        started: &Started,
    ) -> reqwest::Response {
        let mut cookies = self.cookies();
        cookies.push(format!(
            "{}={}",
            ProviderCookie::new(&console_url()).name(),
            started.binding
        ));
        server
            .client
            .post(server.api_url(&format!("/v0/auth/providers/{provider}/finish")))
            .header(header::COOKIE, cookies.join("; "))
            .header("Sec-Fetch-Site", "same-origin")
            .json(&json!({"code": CODE, "state": started.state}))
            .send()
            .await
            .expect("Request failed")
    }

    fn cookies(&self) -> Vec<String> {
        self.session
            .iter()
            .map(session_cookie)
            .chain(
                self.pending.iter().map(|pending| {
                    format!("{}={pending}", PendingCookie::new(&console_url()).name())
                }),
            )
            .collect()
    }
}

/// A fresh sign-in with a provider that has started: its state, and its binding cookie's value.
struct Started {
    state: String,
    binding: String,
}

/// A request from the console, as `browser`.
async fn send(
    server: &TestServer,
    method: Method,
    path: &str,
    browser: &Browser,
    body: Option<&Value>,
) -> reqwest::Response {
    let mut request = server
        .client
        .request(method, server.api_url(path))
        .header("Sec-Fetch-Site", "same-origin");
    let cookies = browser.cookies();
    if !cookies.is_empty() {
        request = request.header(header::COOKIE, cookies.join("; "));
    }
    if let Some(bearer) = &browser.bearer {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        );
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    request.send().await.expect("Request failed")
}

/// What a proof may change, row by row: every session, user, and pin.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    sessions: Vec<SessionRow>,
    /// Each user's UUID, email, verified time, and last change.
    users: Vec<(String, String, Option<i64>, i64)>,
    pins: Vec<(i32, i32, String, String)>,
}

/// A session's token hash, user, method, device, created, proved, and last used.
type SessionRow = (String, i32, i32, Option<String>, i64, i64, i64);

impl Snapshot {
    fn of(server: &TestServer) -> Self {
        let mut conn = server.db_conn();
        Self {
            sessions: schema::session::table
                .order(schema::session::id)
                .select((
                    schema::session::token_hash,
                    schema::session::user_id,
                    schema::session::method,
                    schema::session::device,
                    schema::session::created,
                    schema::session::proved,
                    schema::session::last_used,
                ))
                .load(&mut conn)
                .expect("Failed to read the sessions"),
            users: schema::user::table
                .order(schema::user::id)
                .select((
                    schema::user::uuid,
                    schema::user::email,
                    schema::user::email_verified,
                    schema::user::modified,
                ))
                .load(&mut conn)
                .expect("Failed to read the users"),
            pins: schema::provider_pin::table
                .order(schema::provider_pin::id)
                .select((
                    schema::provider_pin::user_id,
                    schema::provider_pin::provider,
                    schema::provider_pin::account_id,
                    schema::provider_pin::handle,
                ))
                .load(&mut conn)
                .expect("Failed to read the pins"),
        }
    }

    /// The same, but for the session of `token` proved at `now`.
    fn proved(mut self, token: &SessionToken, now: DateTime) -> Self {
        let hash = SessionTokenHash::from(token).to_string();
        let session = self
            .sessions
            .iter_mut()
            .find(|session| session.0 == hash)
            .expect("No such session");
        session.5 = now.timestamp();
        self
    }
}

/// When the session of `token` was last proved.
fn proved_of(server: &TestServer, token: &SessionToken) -> DateTime {
    schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(token)))
        .select(schema::session::proved)
        .first(&mut server.db_conn())
        .expect("No such session")
}

fn email_of(server: &TestServer, user: &TestUser) -> String {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::email)
        .first(&mut server.db_conn())
        .expect("No such user")
}

fn is_admin_of(server: &TestServer, user: &TestUser) -> bool {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::admin)
        .first(&mut server.db_conn())
        .expect("No such user")
}

fn is_locked_of(server: &TestServer, user: &TestUser) -> bool {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::locked)
        .first(&mut server.db_conn())
        .expect("No such user")
}

fn pending_sign_ins(server: &TestServer) -> i64 {
    schema::pending_sign_in::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count pending sign-ins")
}

fn provider_states(server: &TestServer) -> i64 {
    schema::provider_state::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count provider sign-ins")
}

/// Pin the account `account_id` of `provider` to `user`.
fn pin(
    server: &TestServer,
    user: &TestUser,
    provider: SignInProvider,
    account_id: &str,
    handle: &str,
) {
    let mut conn = server.db_conn();
    let user_id: i32 = schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::id)
        .first(&mut conn)
        .expect("No such user");
    diesel::insert_into(schema::provider_pin::table)
        .values((
            schema::provider_pin::user_id.eq(user_id),
            schema::provider_pin::provider.eq(provider),
            schema::provider_pin::account_id.eq(account_id),
            schema::provider_pin::handle.eq(handle),
            schema::provider_pin::created.eq(0i64),
        ))
        .execute(&mut conn)
        .expect("Failed to pin the account");
}

/// The providers `user` can sign in again with, from a session of theirs.
async fn providers(server: &TestServer, user: &TestUser) -> Vec<SignInProvider> {
    let browser = Browser::signed_in(&server.session(user));
    let resp = send(server, Method::GET, "/v0/auth/fresh", &browser, None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let JsonFreshSignIn { providers } = resp.json().await.expect("Invalid answer");
    providers
}

async fn assert_fresh_sign_in_needed(resp: reqwest::Response, action: Action) {
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{action:?}");
    assert_eq!(error(resp).await.0, "fresh_sign_in", "{action:?}");
}

async fn assert_ended(resp: reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error(resp).await.0, "sign_in_ended");
}

/// The error an answer carries: its code and its message.
async fn error(resp: reqwest::Response) -> (String, String) {
    let json: Value = resp.json().await.expect("Failed to parse");
    let field = |name| {
        json.get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (field("error_code"), field("message"))
}

fn header_value<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// Every `Set-Cookie` an answer carries.
fn cookies(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("Invalid header").to_owned())
        .collect()
}

/// The value a named cookie is set to, unless it is cleared.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    cookies(headers).into_iter().find_map(|cookie| {
        let (value, attributes) = cookie.strip_prefix(&format!("{name}="))?.split_once(';')?;
        (!attributes.contains("Max-Age=0")).then(|| value.to_owned())
    })
}

fn pending_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, PendingCookie::new(&console_url()).name())
}

fn session_cookie_set(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SessionCookie::new(&console_url()).name())
}

fn pending_clear() -> String {
    PendingCookie::new(&console_url())
        .clear()
        .to_str()
        .expect("Invalid header")
        .to_owned()
}

fn provider_clear() -> String {
    ProviderCookie::new(&console_url())
        .clear()
        .to_str()
        .expect("Invalid header")
        .to_owned()
}

fn console_url() -> Url {
    "http://localhost:3000".parse().expect("Invalid URL")
}

/// A code other than `code`.
fn wrong_code(code: &SignInCode) -> SignInCode {
    let wrong = if code.as_ref() == "000000" {
        "000001"
    } else {
        "000000"
    };
    wrong.parse().expect("Invalid code")
}

/// GitHub's answers to one sign-in of the account `id`, whose primary verified email is `email`.
fn github_account(id: u64, login: &str, email: &str) -> Vec<(u16, String)> {
    vec![
        (
            200,
            json!({"access_token": "github-token-marker", "token_type": "bearer", "scope": ""})
                .to_string(),
        ),
        (
            200,
            json!({"id": id, "login": login, "name": null}).to_string(),
        ),
        (
            200,
            json!([{"email": email, "primary": true, "verified": true, "visibility": null}])
                .to_string(),
        ),
    ]
}

/// A clock the test moves, from now.
#[derive(Clone)]
struct TestClock(Arc<AtomicI64>);

impl TestClock {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(DateTime::now().timestamp())))
    }

    fn clock(&self) -> Clock {
        let now = Arc::clone(&self.0);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(now.load(Ordering::SeqCst)).expect("Invalid time")
        }))
    }

    fn now(&self) -> DateTime {
        DateTime::try_from(self.0.load(Ordering::SeqCst)).expect("Invalid time")
    }

    fn set(&self, start: DateTime, elapsed: i64) {
        self.0.store(start.timestamp() + elapsed, Ordering::SeqCst);
    }

    fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// A licensed server whose clock is in the test's hands and whose emails are in its log, and,
/// with providers, which signs in with GitHub and Google.
struct Harness {
    server: TestServer,
    clock: TestClock,
    log: LogCapture,
    /// The server admin, its first user.
    admin: TestUser,
}

impl Harness {
    async fn new() -> Self {
        Self::with_options(TestServerOptions::default()).await
    }

    async fn with_options(options: TestServerOptions) -> Self {
        let clock = TestClock::new();
        let log = LogCapture::default();
        let server = TestServer::new_with_options(TestServerOptions {
            clock: Some(clock.clock()),
            log: Some(log.logger()),
            ..options
        })
        .await;
        let admin = server.signup("Admin", "admin@license.example").await;
        let org = server.create_org(&admin, "Licensed").await;
        server
            .license_org(&admin, &org, PlanLevel::Enterprise)
            .await;
        Self {
            server,
            clock,
            log,
            admin,
        }
    }

    /// No test here reaches Google, so its base answers nothing.
    async fn with_providers(github: &Url) -> Self {
        let google = Unscripted::bind().await.base();
        Box::pin(Self::with_options(TestServerOptions {
            github_client: Some(github_client().with_base_url(github.clone())),
            google_client: Some(google_client().with_base_url(&google)),
            ..TestServerOptions::default()
        }))
        .await
    }
}
