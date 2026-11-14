#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in by email: the pending sign-in, its code and its link, Resend,
//! Turnstile, the limits, and the email.

use std::{
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
};

use async_trait::async_trait;
use bencher_api_tests::{
    LogCapture, TestServer, TestServerOptions,
    helpers::{oauth_server, seed_oauth_authorization, user_organizations},
    session_cookie,
};
use bencher_json::{
    Clock, DateTime, Email, EmailHash, InviteSecret, JsonSession, JsonSignInAs, JsonSignInEmail,
    JsonSignedIn, LinkSecret, OAuthAuthorizationUuid, OrganizationUuid, PlanLevel, Secret,
    SessionMethod, SessionToken, SessionTokenHash, SignInCode, SignInCodeHash, TRIES_LEFT,
    system::config::{JsonRateLimiting, JsonSmtp},
};
use bencher_schema::{
    context::{Messenger, RateLimiting, Turnstile},
    model::user::{
        session::cookie::{PendingCookie, SessionCookie},
        sign_in::{InsertPendingSignIn, SignInContext},
    },
    schema,
};
use bencher_turnstile::{SiteVerify, SiteVerifyForm, TurnstileError};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use http::{HeaderMap, Method, StatusCode, header};
use serde_json::json;
use slog::Drain as _;

const MINUTE: i64 = 60;
const DAY: i64 = 24 * 60 * MINUTE;
const SAME_ORIGIN: (&str, &str) = ("Sec-Fetch-Site", "same-origin");
const CROSS_SITE: (&str, &str) = ("Sec-Fetch-Site", "cross-site");
const RELAY_SECRET: &str = "a-relay-secret-for-tests";
const FIREFOX_ON_MACOS: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.6; rv:131.0) Gecko/20100101 Firefox/131.0";

// A code and a link each work once, and a proof by either ends both.
#[tokio::test]
async fn code_and_link_each_work_once_and_one_proof_ends_both() {
    let (server, log) = test_server(TestServerOptions::default()).await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();
    let resp = send_json(
        &server,
        "/v0/auth/code",
        Some(&pending),
        &[SAME_ORIGIN, (header::USER_AGENT.as_str(), FIREFOX_ON_MACOS)],
        &json!({"code": code.as_ref()}),
    )
    .await;
    let session = assert_signed_in(&server, &resp).await;
    assert_eq!(session.user.email.as_ref(), "ada@example.com");
    let (device, created, proved): (Option<String>, i64, i64) = schema::session::table
        .select((
            schema::session::device,
            schema::session::created,
            schema::session::proved,
        ))
        .first(&mut server.db_conn())
        .expect("No session");
    assert_eq!(device.as_deref(), Some("Firefox on macOS"));
    assert_eq!(proved, created);
    assert_ended(redeem_code(&server, Some(&pending), &code).await).await;
    assert_ended(redeem_link(&server, Some(&pending), &link, true).await).await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();
    assert_signed_in(
        &server,
        &redeem_link(&server, Some(&pending), &link, false).await,
    )
    .await;
    assert_ended(redeem_link(&server, Some(&pending), &link, true).await).await;
    assert_ended(redeem_code(&server, Some(&pending), &code).await).await;
}

// Each wrong code counts a try and says how many are left. The fifth ends the sign-in, so even the
// right code after it is refused.
#[tokio::test]
async fn fifth_wrong_code_ends_the_sign_in() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let wrong = wrong_code(&code);

    for (tries_left, left) in [
        ("4", "4 tries left"),
        ("3", "3 tries left"),
        ("2", "2 tries left"),
        ("1", "1 try left"),
    ] {
        let resp = redeem_code(&server, Some(&pending), &wrong).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
        assert_eq!(header_value(resp.headers(), TRIES_LEFT), Some(tries_left));
        let (refusal, message) = error(resp).await;
        assert_eq!(refusal, "wrong_code");
        assert!(message.contains(left), "{left}");
    }
    let resp = redeem_code(&server, Some(&pending), &wrong).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(set_cookies(resp.headers()), [pending_clear()]);
    assert_eq!(header_value(resp.headers(), TRIES_LEFT), Some("0"));
    let (refusal, message) = error(resp).await;
    assert_eq!(refusal, "no_tries_left");
    assert!(message.contains("No tries left"));

    assert_ended(redeem_code(&server, Some(&pending), &code).await).await;
    assert_eq!(pending_sign_ins(&server), 0);
    assert_eq!(sessions(&server), 0);
}

// Single use and the try cap hold under concurrent requests: of many wrong codes at once only five
// count before the sign-in ends, and of many right codes at once only one signs in.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_codes_keep_single_use_and_the_try_cap() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let mut statuses = concurrently(&server, 12, &pending, &wrong_code(&code)).await;
    statuses.sort();
    assert_eq!(
        statuses,
        [
            [StatusCode::UNAUTHORIZED; 7].as_slice(),
            &[StatusCode::FORBIDDEN; 5]
        ]
        .concat()
    );
    assert_ended(redeem_code(&server, Some(&pending), &code).await).await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let mut statuses = concurrently(&server, 12, &pending, &code).await;
    statuses.sort();
    assert_eq!(
        statuses,
        [[StatusCode::OK].as_slice(), &[StatusCode::UNAUTHORIZED; 11]].concat()
    );
    assert_eq!(sessions(&server), 1);
}

// A code signs in only the browser that holds its own pending cookie.
#[tokio::test]
async fn code_needs_its_own_pending_cookie() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let other = start(&server, "grace@example.com", None).await;

    assert_ended(redeem_code(&server, None, &code).await).await;
    let resp = redeem_code(&server, Some(&other), &code).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error(resp).await.0, "wrong_code");

    let ended = start(&server, "ada@example.com", None).await;
    let (ended_code, _link) = log.sign_in();
    let resp = send(
        &server,
        Method::DELETE,
        "/v0/auth/email",
        Some(&ended),
        &[SAME_ORIGIN],
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(set_cookies(resp.headers()), [pending_clear()]);
    assert_ended(redeem_code(&server, Some(&ended), &ended_code).await).await;

    assert_eq!(sessions(&server), 0);
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;
}

// A link signs in at once in the browser that asked for it. Anywhere else it names the address and
// changes nothing, so a later code still works, until the reader confirms.
#[tokio::test]
async fn link_asks_before_it_signs_in_another_browser() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();
    let other = start(&server, "grace@example.com", None).await;

    for cookie in [None, Some(other.as_str())] {
        let resp = redeem_link(&server, cookie, &link, false).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
        let json: serde_json::Value = resp.json().await.expect("Failed to parse");
        assert_eq!(json, json!({"email": "ada@example.com"}));
    }
    assert_eq!(sessions(&server), 0);
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;

    start(&server, "ada@example.com", None).await;
    let (_code, link) = log.sign_in();
    let resp = redeem_link(&server, Some(&other), &link, true).await;
    assert_signed_in(&server, &resp).await;
    let json: JsonSignInAs = resp.json().await.expect("Failed to parse");
    assert_eq!(json.email.as_ref(), "ada@example.com");
    assert_eq!(
        json.signed_in.expect("Not signed in").user.email.as_ref(),
        "ada@example.com"
    );
}

// Resend opens 60 seconds after a sign-in starts, ends it, and starts another with a new code and
// a new link.
#[tokio::test]
async fn resend_waits_a_minute_and_replaces_the_code_and_link() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let started = clock.now();
    let old = start(&server, "ada@example.com", None).await;
    let (old_code, old_link) = log.sign_in();

    clock.set(started, MINUTE - 1);
    let resp = resend(&server, &old).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        header_value(resp.headers(), header::RETRY_AFTER.as_str()),
        Some("1")
    );
    assert_eq!(error(resp).await.0, "resend_waiting");
    assert_eq!(log.sign_in_emails().len(), 1);

    clock.set(started, MINUTE);
    let resp = resend(&server, &old).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let new = pending_cookie(resp.headers()).expect("No pending cookie");
    assert_ne!(new, old);
    let json: JsonSignInEmail = resp.json().await.expect("Failed to parse");
    assert_eq!(json.resend_after, Some(60));
    let (new_code, new_link) = log.sign_in();

    assert_ended(redeem_code(&server, Some(&old), &old_code).await).await;
    assert_ended(redeem_link(&server, Some(&old), &old_link, true).await).await;
    assert_ended(resend(&server, &old).await).await;
    let resp = redeem_link(&server, None, &new_link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_signed_in(&server, &redeem_code(&server, Some(&new), &new_code).await).await;
}

// A Resend whose sign-in ends after Resend read it, as a proof or its last try ends it, is refused
// as ended, and the sign-in it started ends too: its email went out, but its code and link are dead.
#[tokio::test]
async fn resend_refuses_once_its_sign_in_ended_meanwhile() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let (pending, _code) = seed_pending(&server, "ada@example.com", MINUTE);
    server
        .db_conn()
        .batch_execute(
            "CREATE TRIGGER ended_meanwhile AFTER INSERT ON pending_sign_in
            BEGIN DELETE FROM pending_sign_in WHERE id != NEW.id; END;",
        )
        .expect("Failed to add the trigger");

    assert_ended(resend(&server, &pending).await).await;
    assert_eq!(log.sign_in_emails().len(), 1);
    let (_code, link) = log.sign_in();
    assert_ended(redeem_link(&server, None, &link, true).await).await;
    assert_eq!(pending_sign_ins(&server), 0);
}

// A pending sign-in ends 30 minutes after it starts, its code and its link with it.
#[tokio::test]
async fn pending_sign_in_ends_after_thirty_minutes() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let started = clock.now();
    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();

    clock.set(started, 30 * MINUTE - 1);
    let resp = redeem_link(&server, None, &link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    clock.set(started, 30 * MINUTE);
    assert_ended(redeem_link(&server, None, &link, true).await).await;
    assert_ended(redeem_code(&server, Some(&pending), &code).await).await;
    assert_ended(resend(&server, &pending).await).await;
}

// A start in a browser that holds a pending sign-in ends it; one in another browser does not.
#[tokio::test]
async fn new_start_in_the_same_browser_ends_the_earlier_one() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let other = start(&server, "ada@example.com", None).await;
    let (other_code, _link) = log.sign_in();

    let first = start(&server, "ada@example.com", None).await;
    let (first_code, first_link) = log.sign_in();
    let second = start(&server, "grace@example.com", Some(&first)).await;
    let (second_code, _link) = log.sign_in();

    assert_ended(redeem_code(&server, Some(&first), &first_code).await).await;
    assert_ended(redeem_link(&server, None, &first_link, true).await).await;
    assert_signed_in(
        &server,
        &redeem_code(&server, Some(&second), &second_code).await,
    )
    .await;
    assert_signed_in(
        &server,
        &redeem_code(&server, Some(&other), &other_code).await,
    )
    .await;
}

// An address at its cap gets no email and no new sign-in, from a start or from Resend: the
// browser's earlier sign-in and the address's earlier one still work, and the answer carries the
// wait.
#[tokio::test]
async fn address_at_its_cap_keeps_its_earlier_sign_in() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        rate_limiting: Some(RateLimiting::default()),
        ..TestServerOptions::default()
    })
    .await;
    start(&server, "ada@example.com", None).await;
    let earlier = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let browser = start(&server, "grace@example.com", None).await;
    let (browser_code, _link) = log.sign_in();
    let rows = pending_sign_ins(&server);

    let resp = start_raw(&server, "ada@example.com", Some(&browser), &[SAME_ORIGIN]).await;
    assert_capped(resp).await;
    clock.set(clock.now(), MINUTE);
    assert_capped(resend(&server, &earlier).await).await;

    assert_eq!(log.sign_in_emails().len(), 3);
    assert_eq!(pending_sign_ins(&server), rows);
    assert_signed_in(&server, &redeem_code(&server, Some(&earlier), &code).await).await;
    assert_signed_in(
        &server,
        &redeem_code(&server, Some(&browser), &browser_code).await,
    )
    .await;
}

async fn assert_capped(resp: reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    let json: serde_json::Value = resp.json().await.expect("Failed to parse");
    let wait = json
        .get("wait")
        .and_then(serde_json::Value::as_u64)
        .expect("No wait");
    assert!((1..=65).contains(&wait), "{wait}");
    assert_eq!(json, json!({"email": "ada@example.com", "wait": wait}));
}

// Every endpoint charges the per-IP attempts limit, keyed on the address the relay forwards.
#[tokio::test]
async fn every_endpoint_charges_the_trusted_client_ip() {
    let clock = TestClock::new();
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"attempts": {"minute": 5}}}))
            .expect("Invalid rate limiting");
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        relay_secret: Some(RELAY_SECRET.parse::<Secret>().expect("Invalid secret")),
        rate_limiting: Some(RateLimiting::from(rate_limiting)),
        ..TestServerOptions::default()
    })
    .await;
    let reader = relayed("203.0.113.7");
    let resp = start_raw(&server, "ada@example.com", None, &reader).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let pending = pending_cookie(resp.headers()).expect("No pending cookie");
    let (code, link) = log.sign_in();
    clock.set(clock.now(), MINUTE);
    for (method, path, body) in [
        (Method::POST, "/v0/auth/email/resend", json!({})),
        (
            Method::POST,
            "/v0/auth/code",
            json!({"code": code.as_ref()}),
        ),
        (
            Method::POST,
            "/v0/auth/link",
            json!({"secret": link.as_ref(), "confirm": false}),
        ),
        (Method::DELETE, "/v0/auth/email", json!({})),
    ] {
        let resp = send(&server, method, path, Some(&pending), &reader, Some(&body)).await;
        assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
    }
    let resp = start_raw(&server, "grace@example.com", None, &reader).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error(resp).await.0, "too_many_attempts");

    let resp = start_raw(&server, "grace@example.com", None, &relayed("203.0.113.8")).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// A refusal for too many attempts says in Retry-After how long until the reader's address has room
// again, by the window that is full.
#[tokio::test]
async fn too_many_attempts_says_when_to_retry() {
    for (limits, retry_after) in [
        (json!({"minute": 1}), 61..=65),
        (json!({"hour": 1}), 3601..=3900),
    ] {
        let rate_limiting: JsonRateLimiting =
            serde_json::from_value(json!({"public": {"attempts": limits}}))
                .expect("Invalid rate limiting");
        let (server, _log) = test_server(TestServerOptions {
            relay_secret: Some(RELAY_SECRET.parse::<Secret>().expect("Invalid secret")),
            rate_limiting: Some(RateLimiting::from(rate_limiting)),
            ..TestServerOptions::default()
        })
        .await;
        let reader = relayed("203.0.113.7");
        let resp = start_raw(&server, "ada@example.com", None, &reader).await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED, "{limits}");
        let resp = start_raw(&server, "ada@example.com", None, &reader).await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{limits}");
        let wait: u64 = header_value(resp.headers(), header::RETRY_AFTER.as_str())
            .expect("No Retry-After")
            .parse()
            .expect("Invalid Retry-After");
        assert!(retry_after.contains(&wait), "{limits}: {wait}");
        assert_eq!(error(resp).await.0, "too_many_attempts");
    }
}

// An address with an account, one without, and a locked one get the same answer and cookie; only
// the email differs, and the locked account's carries no code and no link.
#[tokio::test]
async fn every_address_gets_the_same_answer() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    server.signup("Ada", "ada@example.com").await;
    let locked = server.signup("Grace", "grace@example.com").await;
    lock(&server, &locked.email);

    let mut answers = Vec::new();
    for (email, lines) in [
        (
            "ada@example.com",
            ["Subject: Your Bencher code is ", "Sign in to Bencher"],
        ),
        (
            "new@example.com",
            ["Subject: Your Bencher code is ", "Sign up for Bencher"],
        ),
        (
            "grace@example.com",
            [
                "Subject: Your Bencher account is locked",
                "Someone asked to sign in with this address.",
            ],
        ),
    ] {
        let resp = start_raw(&server, email, None, &[SAME_ORIGIN]).await;
        let status = resp.status();
        let cookies = set_cookies(resp.headers())
            .iter()
            .map(|cookie| without_value(cookie))
            .collect::<Vec<_>>();
        let json: serde_json::Value = resp.json().await.expect("Failed to parse");
        assert_eq!(json, json!({"email": email, "resend_after": 60}), "{email}");
        answers.push((status, cookies));

        let sent = log.sign_in_emails().pop().expect("No email");
        assert!(sent.contains(&format!("<{email}>")), "{sent}");
        for line in lines {
            assert!(sent.contains(line), "{sent}");
        }
    }
    let locked_email = log.sign_in_emails().pop().expect("No email");
    assert!(!locked_email.contains("/auth/confirm#"), "{locked_email}");
    assert!(!locked_email.contains("code"), "{locked_email}");
    assert!(
        locked_email
            .contains("To unlock the account, contact the administrator of this Bencher server."),
        "{locked_email}"
    );

    let expected = (
        StatusCode::ACCEPTED,
        vec![
            "__Host-bencher_pending=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=1800"
                .to_owned(),
        ],
    );
    for answer in answers {
        assert_eq!(answer, expected);
    }
}

// No account or organization exists until the address is proven. Then the account is made as a
// signup makes one, the first on the server is its admin, and the admins hear of each after it.
#[tokio::test]
async fn new_account_is_made_only_at_proof() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada.lovelace@example.com", None).await;
    assert_eq!((users(&server), organizations(&server)), (0, 0));

    let (code, _link) = log.sign_in();
    let user = signed_in(&server, redeem_code(&server, Some(&pending), &code).await).await;
    assert!(user.new);
    assert!(user.user.admin, "the first user is admin");
    assert_eq!(user.user.name.as_ref(), "ada.lovelace");
    assert_eq!(user.user.email.as_ref(), "ada.lovelace@example.com");
    assert_eq!(
        user_organizations(&server, user.user.uuid),
        [OrganizationUuid::from(user.user.uuid)]
    );
    assert!(email_verified(&server, &user.user.email));
    assert_eq!(admin_notices(&log), 0);

    let pending = start(&server, "grace@example.com", None).await;
    assert_eq!(users(&server), 1);
    let (code, _link) = log.sign_in();
    let user = signed_in(&server, redeem_code(&server, Some(&pending), &code).await).await;
    assert!(user.new);
    assert!(!user.user.admin);
    assert_eq!(admin_notices(&log), 1);

    let pending = start(&server, "grace@example.com", None).await;
    let (code, _link) = log.sign_in();
    let user = signed_in(&server, redeem_code(&server, Some(&pending), &code).await).await;
    assert!(!user.new);
    assert_eq!(admin_notices(&log), 1);
    assert_eq!(users(&server), 2);
}

// An invite, a claim, a plan, and an app's request to connect ride from the start to the proof,
// for a new account and for an existing one.
#[tokio::test]
async fn context_is_carried_to_the_proof() {
    let (server, log) = test_server(TestServerOptions {
        oauth: Some(oauth_server()),
        ..TestServerOptions::default()
    })
    .await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let invited = server.create_org(&leader, "Invited").await;
    server.signup("Member", "member@example.com").await;
    let authorization = seed_oauth_authorization(&server, DateTime::now());

    for email in ["new@example.com", "member@example.com"] {
        let invite = server.invite(&leader, &invited, email);
        let user = prove_with(
            &server,
            &log,
            &json!({
                "email": email,
                "invite": invite.as_ref(),
                "plan": "pro",
                "authorization": authorization.to_string(),
            }),
        )
        .await;
        assert_eq!(user.plan, Some(PlanLevel::Pro));
        assert_eq!(user.authorization, Some(authorization));
        assert!(user_organizations(&server, user.user.uuid).contains(&invited.uuid));
        if email == "new@example.com" {
            assert_eq!(
                user_organizations(&server, user.user.uuid),
                [invited.uuid],
                "an invited signup makes no personal organization"
            );
        }
    }

    for email in ["claimer@example.com", "member@example.com"] {
        let unclaimed = unclaimed_organization(&server, email);
        let user = prove_with(&server, &log, &json!({"email": email, "claim": unclaimed})).await;
        assert_eq!(
            role(&server, &user, unclaimed).as_deref(),
            Some("leader"),
            "{email}"
        );
    }

    let gone = OAuthAuthorizationUuid::new();
    let user = prove_with(
        &server,
        &log,
        &json!({"email": "member@example.com", "authorization": gone.to_string()}),
    )
    .await;
    assert_eq!(
        user.authorization, None,
        "a request no longer pending is dropped"
    );
}

// A proof is one write: when its session cannot be written, the account is not made and the
// sign-in has not ended, so its code still works once the session can be.
#[tokio::test]
async fn proof_rolls_back_whole() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    server
        .db_conn()
        .batch_execute(
            "CREATE TRIGGER no_session BEFORE INSERT ON session
            BEGIN SELECT RAISE(ABORT, 'no session'); END;",
        )
        .expect("Failed to add the trigger");

    let resp = redeem_code(&server, Some(&pending), &code).await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    assert_eq!((users(&server), organizations(&server)), (0, 0));
    assert_eq!(pending_sign_ins(&server), 1);

    server
        .db_conn()
        .batch_execute("DROP TRIGGER no_session")
        .expect("Failed to drop the trigger");
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;

    // A database error while joining the invite's organization is no stale invite to skip.
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;
    let body = json!({
        "email": "grace@example.com",
        "invite": server.invite(&leader, &org, "grace@example.com").as_ref(),
    });
    let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
    let pending = pending_cookie(resp.headers()).expect("No pending cookie");
    let (code, _link) = log.sign_in();
    server
        .db_conn()
        .batch_execute(&format!(
            "CREATE TRIGGER no_role BEFORE INSERT ON organization_role
            WHEN NEW.organization_id = (SELECT id FROM organization WHERE uuid = '{}')
            BEGIN SELECT RAISE(ABORT, 'no role'); END;",
            org.uuid
        ))
        .expect("Failed to add the trigger");
    let resp = redeem_code(&server, Some(&pending), &code).await;
    assert!(!resp.status().is_success(), "{}", resp.status());
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    assert_eq!(pending_sign_ins(&server), 1);
    server
        .db_conn()
        .batch_execute("DROP TRIGGER no_role")
        .expect("Failed to drop the trigger");
    let user = signed_in(&server, redeem_code(&server, Some(&pending), &code).await).await;
    assert_eq!(user_organizations(&server, user.user.uuid), [org.uuid]);
}

// An invite for another address is refused as the sign-in starts, before any email goes out.
#[tokio::test]
async fn invite_for_another_address_is_refused_at_the_start() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;
    let invite = server.invite(&leader, &org, "invited@example.com");
    let resp = send(
        &server,
        Method::POST,
        "/v0/auth/email",
        None,
        &[SAME_ORIGIN],
        Some(&json!({"email": "intruder@example.com", "invite": invite.as_ref()})),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error(resp).await.0, "invite_for_another_email");
    assert_eq!(log.sign_in_emails(), Vec::<String>::new());
    assert_eq!(pending_sign_ins(&server), 0);
}

// An invitation no row holds, one that has ended, one that has expired, and one whose earlier link
// a second invitation replaced are each the one refusal as the sign-in starts, and the
// invitation's own email starts the sign-in.
#[tokio::test]
async fn invalid_invite_is_refused_at_the_start() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;
    let unknown = InviteSecret::generate().expect("Failed to make a secret");
    let ended = server.invite(&leader, &org, "ended@example.com");
    diesel::delete(schema::invite::table.filter(schema::invite::email.eq("ended@example.com")))
        .execute(&mut server.db_conn())
        .expect("Failed to end the invitation");
    let now = DateTime::now();
    let expired = server.invite_at(
        &leader,
        &org,
        "expired@example.com",
        DateTime::try_from(now.timestamp() - 7 * DAY).expect("Invalid time"),
    );
    let replaced = server.invite(&leader, &org, "replaced@example.com");
    let again = server.invite(&leader, &org, "replaced@example.com");
    for (email, invite) in [
        ("ada@example.com", &unknown),
        ("ended@example.com", &ended),
        ("expired@example.com", &expired),
        ("replaced@example.com", &replaced),
    ] {
        let body = json!({"email": email, "invite": invite.as_ref()});
        let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{email}");
        let (refusal, message) = error(resp).await;
        assert_eq!(refusal, "invalid_invite", "{email}");
        assert_eq!(message, "This invitation is not valid", "{email}");
    }
    assert_eq!(log.sign_in_emails(), Vec::<String>::new());
    assert_eq!(pending_sign_ins(&server), 0);

    let body = json!({"email": "replaced@example.com", "invite": again.as_ref()});
    let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// An account locked after its sign-in started is refused at the proof, which ends the sign-in and
// makes no session.
#[tokio::test]
async fn locked_account_is_refused_at_proof() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let user = server.signup("Ada", "ada@example.com").await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();
    lock(&server, &user.email);

    let resp = redeem_code(&server, Some(&pending), &code).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(set_cookies(resp.headers()), [pending_clear()]);
    let (refusal, message) = error(resp).await;
    assert_eq!(refusal, "locked");
    assert!(
        message.contains("contact the administrator of this Bencher server"),
        "{message}"
    );
    assert_eq!(sessions(&server), 0);
    assert_ended(redeem_link(&server, None, &link, true).await).await;
}

// A proof joins the organization whose SSO domain is the address's domain, for a new account and
// for an existing one.
#[tokio::test]
async fn proof_joins_the_sso_organization() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let leader = server.signup("Leader", "leader@other.example").await;
    let existing = server.signup("Grace", "grace@example.com").await;
    let org = server.create_org(&leader, "Example").await;
    let org_id: i32 = schema::organization::table
        .filter(schema::organization::uuid.eq(org.uuid))
        .select(schema::organization::id)
        .first(&mut server.db_conn())
        .expect("No organization");
    diesel::insert_into(schema::sso::table)
        .values((
            schema::sso::uuid.eq("00000000-0000-4000-8000-0000000055a0"),
            schema::sso::organization_id.eq(org_id),
            schema::sso::domain.eq("example.com"),
            schema::sso::created.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to add an SSO domain");

    let new = prove_with(&server, &log, &json!({"email": "ada@example.com"})).await;
    assert!(user_organizations(&server, new.user.uuid).contains(&org.uuid));
    assert!(!user_organizations(&server, existing.uuid).contains(&org.uuid));
    prove_with(&server, &log, &json!({"email": "grace@example.com"})).await;
    assert!(user_organizations(&server, existing.uuid).contains(&org.uuid));
}

// Where Turnstile is configured, it guards a start and a Resend, with the address the relay
// forwards, and refuses a missing token, a failure, an error, another host, and an answer it cannot
// read.
#[tokio::test]
async fn turnstile_guards_starting_and_resending() {
    let clock = TestClock::new();
    let site_verify = Answer::new(success("localhost"));
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        turnstile: Some(turnstile(site_verify.clone())),
        relay_secret: Some(RELAY_SECRET.parse::<Secret>().expect("Invalid secret")),
        ..TestServerOptions::default()
    })
    .await;
    let reader = relayed("203.0.113.7");
    let without = json!({"email": "ada@example.com"});
    let with = json!({"email": "ada@example.com", "turnstile": "a-turnstile-token"});

    let resp = send_json(&server, "/v0/auth/email", None, &reader, &without).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(error(resp).await.0, "turnstile_failed");
    assert_eq!(log.sign_in_emails(), Vec::<String>::new());
    let resp = send_json(&server, "/v0/auth/email", None, &reader, &with).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let pending = pending_cookie(resp.headers()).expect("No pending cookie");
    assert_eq!(
        *site_verify.remote_ips.lock().expect("Poisoned"),
        [Some("203.0.113.7".parse::<IpAddr>().expect("Invalid IP"))]
    );

    clock.set(clock.now(), MINUTE);
    let path = "/v0/auth/email/resend";
    let resp = send_json(&server, path, Some(&pending), &reader, &json!({})).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let token = json!({"turnstile": "a-turnstile-token"});
    let resp = send_json(&server, path, Some(&pending), &reader, &token).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(log.sign_in_emails().len(), 2);

    for answer in [
        Answer::new(|| {
            Ok(r#"{"success":false,"error-codes":["invalid-input-response"]}"#.to_owned())
        }),
        Answer::new(|| Err(TurnstileError::Status(StatusCode::INTERNAL_SERVER_ERROR))),
        Answer::new(success("bencher.dev")),
        Answer::new(|| Ok("not json".to_owned())),
    ] {
        let (server, log) = test_server(TestServerOptions {
            turnstile: Some(turnstile(answer)),
            ..TestServerOptions::default()
        })
        .await;
        let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &with).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(error(resp).await.0, "turnstile_failed");
        assert_eq!(log.sign_in_emails(), Vec::<String>::new());
        assert_eq!(pending_sign_ins(&server), 0);
    }
}

// Where Turnstile is not configured, no token is asked for.
#[tokio::test]
async fn turnstile_is_not_asked_where_unconfigured() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let resp = start_raw(&server, "ada@example.com", None, &[SAME_ORIGIN]).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(log.sign_in_emails().len(), 1);
}

// Every endpoint refuses a request from another origin, and it changes nothing.
#[tokio::test]
async fn every_endpoint_needs_the_console_origin() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let foreign_origin = [("Origin", "https://evil.example")];
    for headers in [&[CROSS_SITE][..], &[][..], &foreign_origin[..]] {
        let resp = start_raw(&server, "ada@example.com", None, headers).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert_eq!(error(resp).await.0, "cross_origin");
    }
    assert_eq!(log.sign_in_emails(), Vec::<String>::new());
    assert_eq!(pending_sign_ins(&server), 0);

    let console_origin = [("Origin", "http://localhost:3000")];
    let resp = start_raw(&server, "ada@example.com", None, &console_origin).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let pending = pending_cookie(resp.headers()).expect("No pending cookie");
    let (code, link) = log.sign_in();
    clock.set(clock.now(), MINUTE);

    for (method, path, body) in [
        (Method::POST, "/v0/auth/email/resend", json!({})),
        (
            Method::POST,
            "/v0/auth/code",
            json!({"code": code.as_ref()}),
        ),
        (
            Method::POST,
            "/v0/auth/link",
            json!({"secret": link.as_ref(), "confirm": true}),
        ),
        (Method::DELETE, "/v0/auth/email", json!({})),
    ] {
        let resp = send(
            &server,
            method,
            path,
            Some(&pending),
            &[CROSS_SITE],
            Some(&body),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
        assert_eq!(set_cookies(resp.headers()), Vec::<String>::new(), "{path}");
        assert_eq!(error(resp).await.0, "cross_origin", "{path}");
    }
    assert_eq!(log.sign_in_emails().len(), 1);
    assert_eq!(sessions(&server), 0);
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;
}

// No answer carries a code, a link's secret, a binding secret, or a session token, but for each
// cookie in its own `Set-Cookie`, and the database holds only their hashes.
#[tokio::test]
async fn no_secret_leaves_its_place() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let mut answers = Vec::new();

    let resp = start_raw(&server, "ada@example.com", None, &[SAME_ORIGIN]).await;
    let first = pending_cookie(resp.headers()).expect("No pending cookie");
    answers.push(answer(resp).await);
    let (first_code, first_link) = log.sign_in();
    clock.set(clock.now(), MINUTE);
    let resp = resend(&server, &first).await;
    let second = pending_cookie(resp.headers()).expect("No pending cookie");
    answers.push(answer(resp).await);
    let (code, link) = log.sign_in();
    answers.push(answer(redeem_code(&server, Some(&second), &wrong_code(&code)).await).await);
    answers.push(answer(redeem_link(&server, None, &link, false).await).await);
    let resp = redeem_code(&server, Some(&second), &code).await;
    let session = session_cookie_value(resp.headers()).expect("No session cookie");
    answers.push(answer(resp).await);

    let secrets = [
        first_code.as_ref(),
        first_link.as_ref(),
        first.as_str(),
        code.as_ref(),
        link.as_ref(),
        second.as_str(),
        session.as_str(),
    ];
    let own_cookies = [
        format!("{}={first};", PendingCookie::new(&console_url()).name()),
        format!("{}={second};", PendingCookie::new(&console_url()).name()),
        format!("{}={session};", SessionCookie::Host.name()),
    ];
    for (index, (headers, body)) in answers.iter().enumerate() {
        for secret in secrets {
            assert!(!body.contains(secret), "answer {index}: {body}");
            for (name, value) in headers {
                let value = String::from_utf8_lossy(value.as_bytes());
                let own_cookie = name == header::SET_COOKIE
                    && own_cookies
                        .iter()
                        .any(|own| value.starts_with(own.as_str()));
                assert!(
                    own_cookie || !value.contains(secret),
                    "answer {index}: {name}: {value}"
                );
            }
        }
    }

    start(&server, "grace@example.com", None).await;
    let (code, link) = log.sign_in();
    let mut stored: Vec<String> = schema::pending_sign_in::table
        .select((
            schema::pending_sign_in::binding_hash,
            schema::pending_sign_in::code_hash,
            schema::pending_sign_in::link_hash,
        ))
        .load::<(String, String, String)>(&mut server.db_conn())
        .expect("Failed to load")
        .into_iter()
        .flat_map(|(binding, code, link)| [binding, code, link])
        .collect();
    stored.extend(
        schema::session::table
            .select(schema::session::token_hash)
            .load::<String>(&mut server.db_conn())
            .expect("Failed to load"),
    );
    assert_eq!(stored.len(), 4);
    for secret in secrets.into_iter().chain([code.as_ref(), link.as_ref()]) {
        for hash in &stored {
            assert_ne!(hash, secret);
            assert!(!hash.contains(secret) || secret.len() == 6, "{hash}");
        }
    }
}

// With no SMTP server, the email, its code and its link included, is logged at info in its place.
#[tokio::test]
async fn without_smtp_the_email_is_logged_at_info() {
    let capture = LogCapture::default();
    let log = slog::Logger::root(
        slog::LevelFilter::new(capture.clone(), slog::Level::Info).ignore_res(),
        slog::o!(),
    );
    let server = TestServer::new_with_options(TestServerOptions {
        log: Some(log),
        ..TestServerOptions::default()
    })
    .await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = capture.sign_in();
    let resp = redeem_link(&server, None, &link, false).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;
}

// With an SMTP server set, the send is awaited, and no log line at any level carries the code, the
// link, or the body.
#[tokio::test]
async fn with_smtp_no_log_line_carries_the_email() {
    let (server, log) = test_server(TestServerOptions {
        messenger: Some(unreachable_smtp()),
        ..TestServerOptions::default()
    })
    .await;

    let resp = start_raw(&server, "ada@example.com", None, &[SAME_ORIGIN]).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    let code_hash: SignInCodeHash = schema::pending_sign_in::table
        .select(schema::pending_sign_in::code_hash)
        .first(&mut server.db_conn())
        .expect("No pending sign-in");
    let code_key = &server.context().code_key;
    let code = (0..1_000_000)
        .map(|code| {
            format!("{code:06}")
                .parse::<SignInCode>()
                .expect("Invalid code")
        })
        .find(|code| code_key.matches(code, &code_hash))
        .expect("No code matches");

    let lines = log.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Sending email") && line.contains("ada@example.com")),
        "the send is logged"
    );
    for line in &lines {
        for secret in [
            code.as_ref(),
            "/auth/confirm#",
            "Your Bencher code",
            "The code and the button work once",
        ] {
            assert!(!line.contains(secret), "{line}");
        }
    }
}

// The per-IP charge comes before Turnstile, so a script with junk tokens is refused once over its
// limit and costs no more Turnstile checks.
#[tokio::test]
async fn per_ip_charge_comes_before_turnstile() {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"attempts": {"minute": 2}}}))
            .expect("Invalid rate limiting");
    let site_verify = Answer::new(|| Ok(r#"{"success":false}"#.to_owned()));
    let (server, _log) = test_server(TestServerOptions {
        turnstile: Some(turnstile(site_verify.clone())),
        relay_secret: Some(RELAY_SECRET.parse::<Secret>().expect("Invalid secret")),
        rate_limiting: Some(RateLimiting::from(rate_limiting)),
        ..TestServerOptions::default()
    })
    .await;
    let junk = json!({"email": "ada@example.com", "turnstile": "a-junk-token"});
    let reader = relayed("203.0.113.7");
    for refusal in ["turnstile_failed", "turnstile_failed", "too_many_attempts"] {
        let resp = send_json(&server, "/v0/auth/email", None, &reader, &junk).await;
        assert_eq!(error(resp).await.0, refusal);
    }
    assert_eq!(site_verify.remote_ips.lock().expect("Poisoned").len(), 2);
}

// A proof ends the session the browser was signed in to, by a code or by a link, and leaves every
// other session alone.
#[tokio::test]
async fn proof_ends_the_session_the_browser_presented() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let user = server.signup("Ada", "ada@example.com").await;
    let other = server.session(&user);
    let presented = server.session(&user);
    let cookie = session_cookie(&presented);

    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let resp = send_json(
        &server,
        "/v0/auth/code",
        Some(&pending),
        &[SAME_ORIGIN, (header::COOKIE.as_str(), &cookie)],
        &json!({"code": code.as_ref()}),
    )
    .await;
    assert_signed_in(&server, &resp).await;
    assert!(!session_exists(&server, &presented));
    assert!(session_exists(&server, &other));
    assert_eq!(sessions(&server), 2);

    let presented = server.session(&user);
    let cookie = session_cookie(&presented);
    start(&server, "ada@example.com", None).await;
    let (_code, link) = log.sign_in();
    let resp = send_json(
        &server,
        "/v0/auth/link",
        None,
        &[SAME_ORIGIN, (header::COOKIE.as_str(), &cookie)],
        &json!({"secret": link.as_ref(), "confirm": true}),
    )
    .await;
    assert_signed_in(&server, &resp).await;
    assert!(!session_exists(&server, &presented));
    assert!(session_exists(&server, &other));
    assert_eq!(sessions(&server), 3);
}

// An invite or a claim that no longer applies at the proof is skipped, and the reader signs in:
// a claim of an organization someone else claimed, an invite to an organization the user belongs
// to or one that is gone. A skipped invite never falls through to the claim, and a new account
// whose invite was skipped still gets its own organization.
#[tokio::test]
async fn stale_invite_or_claim_is_skipped() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let existing = server.signup("Grace", "grace@example.com").await;

    let claimed = unclaimed_organization(&server, "claimed@example.com");
    let first = prove_with(
        &server,
        &log,
        &json!({"email": "claimer@example.com", "claim": claimed}),
    )
    .await;
    let roles = organization_roles(&server, claimed);
    for email in ["new@example.com", "grace@example.com"] {
        let user = prove_with(&server, &log, &json!({"email": email, "claim": claimed})).await;
        assert!(
            !user_organizations(&server, user.user.uuid).contains(&claimed),
            "{email}"
        );
    }
    assert_eq!(organization_roles(&server, claimed), roles);
    assert_eq!(role(&server, &first, claimed).as_deref(), Some("leader"));
    assert_eq!(
        user_organizations(
            &server,
            prove_with(&server, &log, &json!({"email": "new@example.com"}))
                .await
                .user
                .uuid
        )
        .len(),
        1,
        "the new account got its own organization"
    );

    let member_of = server.create_org(&leader, "Member Of").await;
    let invite = server.invite(&leader, &member_of, "leader@example.com");
    prove_with(
        &server,
        &log,
        &json!({"email": "leader@example.com", "invite": invite.as_ref()}),
    )
    .await;
    assert_eq!(invites(&server), 0, "a member's proof ends the invitation");

    let gone = server.create_org(&leader, "Gone").await;
    let unclaimed = unclaimed_organization(&server, "unclaimed@example.com");
    let mut started = Vec::new();
    for email in ["fresh@example.com", "grace@example.com"] {
        let body = json!({
            "email": email,
            "invite": server.invite(&leader, &gone, email).as_ref(),
            "claim": unclaimed,
        });
        let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
        let pending = pending_cookie(resp.headers()).expect("No pending cookie");
        started.push((email, pending, log.sign_in().0));
    }
    diesel::update(schema::organization::table.filter(schema::organization::uuid.eq(gone.uuid)))
        .set(schema::organization::deleted.eq(Some(0i64)))
        .execute(&mut server.db_conn())
        .expect("Failed to delete the organization");
    for (email, pending, code) in started {
        let user = signed_in(&server, redeem_code(&server, Some(&pending), &code).await).await;
        let organizations = user_organizations(&server, user.user.uuid);
        assert!(!organizations.contains(&gone.uuid), "{email}");
        assert!(!organizations.contains(&unclaimed), "{email}");
        if email == "fresh@example.com" {
            assert_eq!(organizations, [OrganizationUuid::from(user.user.uuid)]);
        }
    }
    assert_eq!(organization_roles(&server, unclaimed), Vec::<String>::new());
    let notice = log
        .lines()
        .into_iter()
        .rfind(|line| line.contains("New Bencher User"))
        .expect("No admin notice");
    assert!(notice.contains("fresh@example.com"), "{notice}");
    assert!(notice.contains("has joined"), "{notice}");
    assert_eq!(user_organizations(&server, existing.uuid).len(), 1);
}

// An invitation joins at the proof by link as by code, and a second invitation to the address
// keeps it for a sign-in its earlier link started. One revoked or expired while the sign-in waits is
// skipped, and the reader signs in, with an organization of their own.
#[tokio::test]
async fn invite_joins_at_its_proof_only_while_it_applies() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let started = clock.now();
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;

    let earlier = server.invite(&leader, &org, "grace@example.com");
    let grace = start_with(&server, "grace@example.com", &earlier).await;
    let (_code, link) = log.sign_in();
    server.invite(&leader, &org, "grace@example.com");
    let session = assert_signed_in(
        &server,
        &redeem_link(&server, Some(&grace), &link, false).await,
    )
    .await;
    assert_eq!(user_organizations(&server, session.user.uuid), [org.uuid]);
    assert_eq!(invites(&server), 0);

    let revoked = server.invite(&leader, &org, "alan@example.com");
    let alan = start_with(&server, "alan@example.com", &revoked).await;
    let (code, _link) = log.sign_in();
    let uuid: String = schema::invite::table
        .filter(schema::invite::email.eq("alan@example.com"))
        .select(schema::invite::uuid)
        .first(&mut server.db_conn())
        .expect("No invitation");
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}/invites/{uuid}", org.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&leader.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let carried: Option<i32> = schema::pending_sign_in::table
        .filter(schema::pending_sign_in::email.eq("alan@example.com"))
        .select(schema::pending_sign_in::invite_id)
        .first(&mut server.db_conn())
        .expect("No pending sign-in");
    assert_eq!(
        carried, None,
        "a revoke drops the invitation the sign-in carries"
    );
    let user = signed_in(&server, redeem_code(&server, Some(&alan), &code).await).await;
    assert_eq!(
        user_organizations(&server, user.user.uuid),
        [OrganizationUuid::from(user.user.uuid)]
    );

    let expiring = server.invite_at(
        &leader,
        &org,
        "katherine@example.com",
        DateTime::try_from(started.timestamp() - 7 * DAY + 10 * MINUTE).expect("Invalid time"),
    );
    let katherine = start_with(&server, "katherine@example.com", &expiring).await;
    let (code, _link) = log.sign_in();
    clock.set(started, 10 * MINUTE);
    let user = signed_in(&server, redeem_code(&server, Some(&katherine), &code).await).await;
    assert_eq!(
        user_organizations(&server, user.user.uuid),
        [OrganizationUuid::from(user.user.uuid)]
    );
}

// A claim whose organization does not exist is refused as the sign-in starts.
#[tokio::test]
async fn unknown_organization_is_refused_at_the_start() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let body = json!({"email": "ada@example.com", "claim": OrganizationUuid::new()});
    let resp = send_json(&server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(error(resp).await.0, "unknown_organization");
    assert_eq!(log.sign_in_emails(), Vec::<String>::new());
    assert_eq!(pending_sign_ins(&server), 0);
}

// When a new email cannot be sent, the sign-in it would replace, from a start or from Resend,
// still works.
#[tokio::test]
async fn failed_send_keeps_the_earlier_sign_in() {
    let (server, _log) = test_server(TestServerOptions {
        messenger: Some(unreachable_smtp()),
        ..TestServerOptions::default()
    })
    .await;
    server.signup("Ada", "ada@example.com").await;

    let (earlier, code) = seed_pending(&server, "ada@example.com", 0);
    let resp = start_raw(&server, "ada@example.com", Some(&earlier), &[SAME_ORIGIN]).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error(resp).await.0, "email_failed");
    assert_signed_in(&server, &redeem_code(&server, Some(&earlier), &code).await).await;

    let (earlier, code) = seed_pending(&server, "ada@example.com", MINUTE);
    let resp = resend(&server, &earlier).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error(resp).await.0, "email_failed");
    assert_signed_in(&server, &redeem_code(&server, Some(&earlier), &code).await).await;
}

// Wrong codes count against their address across all of its sign-ins, the fifth of each one
// included: after 10, a code in a new sign-in for the address is paused too. Another address
// keeps its codes.
#[tokio::test]
async fn code_budget_spans_every_sign_in_for_the_address() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    spend_code_budget(&server, &log, "ada@example.com").await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    assert_codes_paused(redeem_code(&server, Some(&pending), &wrong_code(&code)).await).await;

    let other = start(&server, "grace@example.com", None).await;
    let (code, _link) = log.sign_in();
    assert_signed_in(&server, &redeem_code(&server, Some(&other), &code).await).await;
}

// Once the budget is spent, the right code is refused too, so a refusal tells nothing about the
// code. The refusal is a 429 whose Retry-After holds the seconds until the oldest of the 10 wrong
// codes leaves its 24 hours.
#[tokio::test]
async fn spent_code_budget_refuses_the_right_code() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let spent = clock.now();
    spend_code_budget(&server, &log, "ada@example.com").await;

    clock.set(spent, 100);
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let retry_after = assert_codes_paused(redeem_code(&server, Some(&pending), &code).await).await;
    assert_eq!(retry_after, DAY - 100);
    assert_eq!(sessions(&server), 0);
}

// The link signs the reader in while codes are paused, so no reader is locked out.
#[tokio::test]
async fn spent_code_budget_leaves_the_link() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    spend_code_budget(&server, &log, "ada@example.com").await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, link) = log.sign_in();
    assert_codes_paused(redeem_code(&server, Some(&pending), &code).await).await;
    assert_signed_in(
        &server,
        &redeem_link(&server, Some(&pending), &link, false).await,
    )
    .await;
}

// A paused code spends no try of its sign-in and counts nothing against the budget, so refusals
// never push the wait out.
#[tokio::test]
async fn paused_code_spends_no_try() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let spent = clock.now();
    spend_code_budget(&server, &log, "ada@example.com").await;

    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let wrong = wrong_code(&code);
    for elapsed in 1..=6 {
        clock.set(spent, elapsed);
        let resp = redeem_code(&server, Some(&pending), &wrong).await;
        assert_eq!(assert_codes_paused(resp).await, DAY - elapsed);
    }
    let tries: Vec<i32> = schema::pending_sign_in::table
        .select(schema::pending_sign_in::tries)
        .load(&mut server.db_conn())
        .expect("Failed to load tries");
    assert_eq!(tries, [0]);
    assert_eq!(wrong_codes(&server).len(), 10);
}

// The budget rolls on the server's clock: codes stay paused until 24 hours after the oldest of
// the 10 wrong codes, and work again from that second.
#[tokio::test]
async fn code_budget_rolls_with_the_context_clock() {
    let clock = TestClock::new();
    let (server, log) = test_server(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let spent = clock.now();
    spend_code_budget(&server, &log, "ada@example.com").await;

    clock.set(spent, DAY - 1);
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    let retry_after = assert_codes_paused(redeem_code(&server, Some(&pending), &code).await).await;
    assert_eq!(retry_after, 1);
    clock.set(spent, DAY);
    assert_signed_in(&server, &redeem_code(&server, Some(&pending), &code).await).await;
}

// The budget holds whatever the server's rate limits: with none, as a self-hosted server with no
// rate limit config gets, licensed or not, and with configured ones.
#[tokio::test]
async fn code_budget_holds_with_or_without_rate_limits() {
    let limits: JsonRateLimiting =
        serde_json::from_value(json!({"email": {"minute": 100, "hour": 100, "day": 100}}))
            .expect("Invalid rate limiting");
    for rate_limiting in [RateLimiting::max(), RateLimiting::from(limits)] {
        let (server, log) = test_server(TestServerOptions {
            rate_limiting: Some(rate_limiting),
            ..TestServerOptions::default()
        })
        .await;
        spend_code_budget(&server, &log, "ada@example.com").await;

        let pending = start(&server, "ada@example.com", None).await;
        let (code, _link) = log.sign_in();
        assert_codes_paused(redeem_code(&server, Some(&pending), &code).await).await;
    }
}

// The budget is kept on disk, so a restart of the API keeps it.
#[tokio::test]
async fn code_budget_survives_a_restart() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    spend_code_budget(&server, &log, "ada@example.com").await;

    let server = server
        .restart(TestServerOptions {
            log: Some(log.logger()),
            ..TestServerOptions::default()
        })
        .await;
    let pending = start(&server, "ada@example.com", None).await;
    let (code, _link) = log.sign_in();
    assert_codes_paused(redeem_code(&server, Some(&pending), &code).await).await;
}

// Wrong codes sent at once never get past the budget: of 30 across 6 sign-ins, 10 count and the
// other 20 are paused.
#[tokio::test(flavor = "multi_thread")]
async fn parallel_wrong_codes_stop_at_the_budget() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    let mut sign_ins = Vec::new();
    for _ in 0..6 {
        let pending = start(&server, "ada@example.com", None).await;
        let (code, _link) = log.sign_in();
        sign_ins.push((pending, wrong_code(&code)));
    }

    let mut requests = tokio::task::JoinSet::new();
    for (pending, wrong) in &sign_ins {
        for _ in 0..5 {
            let request = code_request(&server, pending, wrong);
            requests.spawn(async move { request.send().await.expect("Request failed").status() });
        }
    }
    let statuses = requests.join_all().await;
    let answered = |status| statuses.iter().filter(|answer| **answer == status).count();
    assert_eq!(
        (
            answered(StatusCode::FORBIDDEN),
            answered(StatusCode::TOO_MANY_REQUESTS)
        ),
        (10, 20),
        "{statuses:?}"
    );
    assert_eq!(wrong_codes(&server).len(), 10);
}

// The budget holds the SHA-256 of the normalized address in place of the address.
#[tokio::test]
async fn code_budget_stores_a_hash_of_the_address() {
    let (server, log) = test_server(TestServerOptions::default()).await;
    spend_code_budget(&server, &log, "Ada@Example.com").await;

    let hashes = wrong_codes(&server);
    assert_eq!(hashes.len(), 10);
    for hash in hashes {
        assert_eq!(
            hash.as_ref(),
            "b5fc85e55755f9e0d030a10ab4429b6b2944855f9a0d60077fe832becbc41d72"
        );
    }
}

/// Answers every Turnstile check the same way, and keeps the address each was for.
struct Answer {
    answer: Box<dyn Fn() -> Result<String, TurnstileError> + Send + Sync>,
    remote_ips: Mutex<Vec<Option<IpAddr>>>,
}

impl Answer {
    fn new(
        answer: impl Fn() -> Result<String, TurnstileError> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            answer: Box::new(answer),
            remote_ips: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl SiteVerify for Answer {
    async fn post(&self, form: &SiteVerifyForm) -> Result<String, TurnstileError> {
        self.remote_ips
            .lock()
            .expect("Poisoned")
            .push(form.remote_ip());
        (self.answer)()
    }
}

fn success(
    hostname: &'static str,
) -> impl Fn() -> Result<String, TurnstileError> + Send + Sync + 'static {
    move || Ok(format!(r#"{{"success":true,"hostname":"{hostname}"}}"#))
}

fn turnstile(site_verify: Arc<dyn SiteVerify>) -> Turnstile {
    Turnstile::with_site_verify(
        "a-turnstile-secret".parse().expect("Invalid secret"),
        &console_url(),
        site_verify,
    )
}

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
}

/// A test server whose emails are read from its log.
async fn test_server(options: TestServerOptions) -> (TestServer, LogCapture) {
    let log = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        log: Some(log.logger()),
        ..options
    })
    .await;
    (server, log)
}

fn console_url() -> url::Url {
    "http://localhost:3000".parse().expect("Invalid URL")
}

/// The headers of a request from the console that the relay forwards for the reader at `ip`.
fn relayed(ip: &'static str) -> [(&'static str, &'static str); 3] {
    [
        SAME_ORIGIN,
        (bencher_json::RELAY_SECRET, RELAY_SECRET),
        (bencher_json::RELAY_CLIENT_IP, ip),
    ]
}

/// Start a sign-in from the console, returning the browser's pending cookie.
async fn start(server: &TestServer, email: &str, pending: Option<&str>) -> String {
    let resp = start_raw(server, email, pending, &[SAME_ORIGIN]).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    pending_cookie(resp.headers()).expect("No pending cookie")
}

async fn start_raw(
    server: &TestServer,
    email: &str,
    pending: Option<&str>,
    headers: &[(&str, &str)],
) -> reqwest::Response {
    send_json(
        server,
        "/v0/auth/email",
        pending,
        headers,
        &json!({"email": email}),
    )
    .await
}

/// Start a sign-in for `email` that carries `invite`, and return its pending cookie.
async fn start_with(server: &TestServer, email: &str, invite: &InviteSecret) -> String {
    let body = json!({"email": email, "invite": invite.as_ref()});
    let resp = send_json(server, "/v0/auth/email", None, &[SAME_ORIGIN], &body).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "{body}");
    pending_cookie(resp.headers()).expect("No pending cookie")
}

/// A whole sign-in by code with `body` as its start, returning what the proof answers.
async fn prove_with(
    server: &TestServer,
    log: &LogCapture,
    body: &serde_json::Value,
) -> JsonSignedIn {
    let resp = send_json(server, "/v0/auth/email", None, &[SAME_ORIGIN], body).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "{body}");
    let pending = pending_cookie(resp.headers()).expect("No pending cookie");
    let (code, _link) = log.sign_in();
    signed_in(server, redeem_code(server, Some(&pending), &code).await).await
}

async fn resend(server: &TestServer, pending: &str) -> reqwest::Response {
    send_json(
        server,
        "/v0/auth/email/resend",
        Some(pending),
        &[SAME_ORIGIN],
        &json!({}),
    )
    .await
}

/// The statuses of `n` requests that redeem `code` at once.
async fn concurrently(
    server: &TestServer,
    n: usize,
    pending: &str,
    code: &SignInCode,
) -> Vec<StatusCode> {
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..n {
        let request = code_request(server, pending, code);
        requests.spawn(async move { request.send().await.expect("Request failed").status() });
    }
    requests.join_all().await
}

/// A request from the console that redeems `code` in the browser holding `pending`.
fn code_request(server: &TestServer, pending: &str, code: &SignInCode) -> reqwest::RequestBuilder {
    server
        .client
        .post(server.api_url("/v0/auth/code"))
        .header(
            header::COOKIE,
            format!("{}={pending}", PendingCookie::new(&console_url()).name()),
        )
        .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
        .json(&json!({"code": code.as_ref()}))
}

async fn redeem_code(
    server: &TestServer,
    pending: Option<&str>,
    code: &SignInCode,
) -> reqwest::Response {
    send_json(
        server,
        "/v0/auth/code",
        pending,
        &[SAME_ORIGIN],
        &json!({"code": code.as_ref()}),
    )
    .await
}

async fn redeem_link(
    server: &TestServer,
    pending: Option<&str>,
    link: &LinkSecret,
    confirm: bool,
) -> reqwest::Response {
    send_json(
        server,
        "/v0/auth/link",
        pending,
        &[SAME_ORIGIN],
        &json!({"secret": link.as_ref(), "confirm": confirm}),
    )
    .await
}

async fn send_json(
    server: &TestServer,
    path: &str,
    pending: Option<&str>,
    headers: &[(&str, &str)],
    body: &serde_json::Value,
) -> reqwest::Response {
    send(server, Method::POST, path, pending, headers, Some(body)).await
}

async fn send(
    server: &TestServer,
    method: Method,
    path: &str,
    pending: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<&serde_json::Value>,
) -> reqwest::Response {
    let mut request = server.client.request(method, server.api_url(path));
    if let Some(pending) = pending {
        request = request.header(
            header::COOKIE,
            format!("{}={pending}", PendingCookie::new(&console_url()).name()),
        );
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    if let Some(body) = body {
        request = request.json(body);
    }
    request.send().await.expect("Request failed")
}

fn header_value<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn set_cookies(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("Invalid header").to_owned())
        .collect()
}

/// The value a named cookie is set to, unless it is cleared.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    set_cookies(headers).into_iter().find_map(|cookie| {
        let (value, attributes) = cookie.strip_prefix(&format!("{name}="))?.split_once(';')?;
        (!attributes.contains("Max-Age=0")).then(|| value.to_owned())
    })
}

fn pending_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, PendingCookie::new(&console_url()).name())
}

fn session_cookie_value(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SessionCookie::Host.name())
}

fn pending_clear() -> String {
    PendingCookie::new(&console_url())
        .clear()
        .to_str()
        .expect("Invalid header")
        .to_owned()
}

/// A `Set-Cookie` value with its cookie's value taken out.
fn without_value(cookie: &str) -> String {
    let (name, rest) = cookie.split_once('=').expect("No cookie value");
    let (_, attributes) = rest.split_once(';').expect("No attributes");
    format!("{name}=;{attributes}")
}

/// Assert that the answer signed the browser in: its session cookie reads an email session, and
/// the pending cookie is cleared.
async fn assert_signed_in(server: &TestServer, resp: &reqwest::Response) -> JsonSession {
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(set_cookies(resp.headers()).contains(&pending_clear()));
    let token: SessionToken = session_cookie_value(resp.headers())
        .expect("No session cookie")
        .parse()
        .expect("Invalid session token");
    let session = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(header::COOKIE, session_cookie(&token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(session.status(), StatusCode::OK);
    let session: JsonSession = session.json().await.expect("Failed to parse");
    assert_eq!(session.method, SessionMethod::Email);
    session
}

async fn assert_ended(resp: reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    assert_eq!(error(resp).await.0, "sign_in_ended");
}

/// Spend the code budget of `email` with 10 wrong codes, 5 in each of 2 sign-ins, each refused
/// as a wrong code.
async fn spend_code_budget(server: &TestServer, log: &LogCapture, email: &str) {
    for _ in 0..2 {
        let pending = start(server, email, None).await;
        let (code, _link) = log.sign_in();
        let wrong = wrong_code(&code);
        for _ in 0..5 {
            let resp = redeem_code(server, Some(&pending), &wrong).await;
            assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        }
    }
}

/// Assert that the answer refused a code because its address's budget is spent, and keeps its
/// sign-in, returning the seconds of its Retry-After.
async fn assert_codes_paused(resp: reqwest::Response) -> i64 {
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
    assert_eq!(header_value(resp.headers(), TRIES_LEFT), None);
    let retry_after = header_value(resp.headers(), header::RETRY_AFTER.as_str())
        .expect("No Retry-After")
        .parse()
        .expect("Invalid Retry-After");
    assert_eq!(error(resp).await.0, "codes_paused");
    retry_after
}

async fn signed_in(server: &TestServer, resp: reqwest::Response) -> JsonSignedIn {
    let session = assert_signed_in(server, &resp).await;
    let json: JsonSignedIn = resp.json().await.expect("Failed to parse");
    assert_eq!(session.user.uuid, json.user.uuid);
    json
}

/// The error an answer carries: its code and its message.
async fn error(resp: reqwest::Response) -> (String, String) {
    let json: serde_json::Value = resp.json().await.expect("Failed to parse");
    let field = |name| {
        json.get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (field("error_code"), field("message"))
}

async fn answer(resp: reqwest::Response) -> (HeaderMap, String) {
    let headers = resp.headers().clone();
    (headers, resp.text().await.expect("No body"))
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

/// The invitations not yet accepted or revoked.
fn invites(server: &TestServer) -> i64 {
    schema::invite::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

/// A pending sign-in for `email` that started `ago` seconds ago, recorded directly, with its
/// binding secret and its code.
fn seed_pending(server: &TestServer, email: &str, ago: i64) -> (String, SignInCode) {
    let started = DateTime::try_from(DateTime::now().timestamp() - ago).expect("Invalid time");
    let (insert, secrets) = InsertPendingSignIn::new(
        &server.context().code_key,
        email.parse().expect("Invalid email"),
        SignInContext::default(),
        started,
    )
    .expect("Failed to make a pending sign-in");
    insert
        .insert(&mut server.db_conn(), started)
        .expect("Failed to record a pending sign-in");
    (secrets.binding.as_ref().to_owned(), secrets.code)
}

/// A messenger whose SMTP server refuses every connection.
fn unreachable_smtp() -> Messenger {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let port = closed.local_addr().expect("No address").port();
    drop(closed);
    let smtp: JsonSmtp = serde_json::from_value(json!({
        "hostname": "127.0.0.1",
        "port": port,
        "username": "bencher",
        "secret": "an-smtp-secret",
        "from_name": "Bencher",
        "from_email": "bencher@example.com",
    }))
    .expect("Invalid SMTP config");
    Messenger::from(Some(smtp))
}

fn session_exists(server: &TestServer, token: &SessionToken) -> bool {
    schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(token)))
        .count()
        .get_result::<i64>(&mut server.db_conn())
        .expect("Failed to count")
        > 0
}

/// The roles in an organization.
fn organization_roles(server: &TestServer, org: OrganizationUuid) -> Vec<String> {
    schema::organization_role::table
        .inner_join(schema::organization::table)
        .filter(schema::organization::uuid.eq(org))
        .select(schema::organization_role::role)
        .load(&mut server.db_conn())
        .expect("Failed to load roles")
}

fn lock(server: &TestServer, email: &Email) {
    diesel::update(schema::user::table.filter(schema::user::email.eq(email)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");
}

/// An organization no one has claimed, named for `email` so that each is new.
fn unclaimed_organization(server: &TestServer, email: &str) -> OrganizationUuid {
    let uuid = OrganizationUuid::new();
    let slug = email.replace(['@', '.'], "-");
    diesel::insert_into(schema::organization::table)
        .values((
            schema::organization::uuid.eq(uuid),
            schema::organization::name.eq(&slug),
            schema::organization::slug.eq(&slug),
            schema::organization::created.eq(0i64),
            schema::organization::modified.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to add an organization");
    uuid
}

fn role(server: &TestServer, user: &JsonSignedIn, org: OrganizationUuid) -> Option<String> {
    schema::organization_role::table
        .inner_join(schema::organization::table)
        .inner_join(schema::user::table)
        .filter(schema::organization::uuid.eq(org))
        .filter(schema::user::uuid.eq(user.user.uuid))
        .select(schema::organization_role::role)
        .first(&mut server.db_conn())
        .ok()
}

fn email_verified(server: &TestServer, email: &Email) -> bool {
    schema::user::table
        .filter(schema::user::email.eq(email))
        .select(schema::user::email_verified)
        .first::<Option<i64>>(&mut server.db_conn())
        .expect("No user")
        .is_some()
}

fn admin_notices(log: &LogCapture) -> usize {
    log.lines()
        .iter()
        .filter(|line| line.contains("Subject: 🐰 New Bencher User"))
        .count()
}

/// The address hash of every wrong code the budget holds.
fn wrong_codes(server: &TestServer) -> Vec<EmailHash> {
    schema::wrong_code::table
        .select(schema::wrong_code::email_hash)
        .load(&mut server.db_conn())
        .expect("Failed to load wrong codes")
}

fn pending_sign_ins(server: &TestServer) -> i64 {
    schema::pending_sign_in::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn sessions(server: &TestServer) -> i64 {
    schema::session::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn users(server: &TestServer) -> i64 {
    schema::user::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn organizations(server: &TestServer) -> i64 {
    schema::organization::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}
