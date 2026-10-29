#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for browser sessions: the session cookie, the same-origin check,
//! the relay secret, and reading and ending the session.

use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use bencher_api_tests::{TestServer, TestServerOptions, TestUser, session_cookie};
use bencher_json::{
    Clock, DateTime, JsonProject, JsonSession, JsonUser, Secret, SessionMethod, SessionToken,
    SessionTokenHash, ThresholdUuid, system::config::JsonRateLimiting,
};
use bencher_schema::{
    context::RateLimiting,
    model::user::session::{QuerySession, cookie::SessionCookie},
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use http::{HeaderMap, Method, StatusCode, header};
use serde_json::json;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const CONSOLE_ORIGIN: &str = "http://localhost:3000";
const RELAY_SECRET: &str = "a-relay-secret-for-tests";

// A valid cookie authenticates a read and, from the console's own origin, a write,
// on a bearer endpoint and on a public one.
#[tokio::test]
async fn session_cookie_authenticates_reads_and_writes() {
    let server = TestServer::new().await;
    let user = server
        .signup("Cookie User", "cookie-user@example.com")
        .await;
    let org = server.create_org(&user, "Cookie Org").await;
    let project = server.create_project(&user, &org, "Cookie Project").await;
    make_private(&server, project.uuid);
    let token = server.session(&user);
    let project_path = format!("/v0/projects/{}", project.slug);

    let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json_user: JsonUser = resp.json().await.expect("Failed to parse user");
    assert_eq!(json_user.uuid, user.uuid);

    let resp = get(&server, &project_path, Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = get(&server, &project_path, None, &[]).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = rename(&server, &user, Some(&token), &[same_origin()], "Renamed").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json_user: JsonUser = resp.json().await.expect("Failed to parse user");
    assert_eq!(json_user.name.as_ref(), "Renamed");
}

// A request with no credential at all is unauthenticated.
#[tokio::test]
async fn no_credential_is_unauthorized() {
    let server = TestServer::new().await;
    let user = server.signup("Bare User", "bare-user@example.com").await;

    let resp = get(&server, &user_path(&user), None, &[]).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(cleared_cookie(resp.headers()).is_none());
}

// An unknown, malformed, idle, ended, or locked user's cookie is refused and cleared.
#[tokio::test]
async fn dead_session_cookies_are_refused() {
    let clock = TestClock::new();
    let server = server_with_clock(&clock).await;
    let user = server.signup("Dead User", "dead-user@example.com").await;
    let path = user_path(&user);
    let start = clock.now();

    let unknown = SessionToken::generate().expect("Failed to generate a token");
    assert_dead(&get(&server, &path, Some(&unknown), &[]).await);
    let malformed = server
        .client
        .get(server.api_url(&path))
        .header(
            header::COOKIE,
            format!("{}=not-a-token", SessionCookie::Host.name()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_dead(&malformed);

    // Unused for 29 days lives; unused for 31 days has ended.
    let idle_live = server.session_at(&user, start);
    let idle_dead = server.session_at(&user, start);
    clock.set(start, 29 * DAY);
    assert_eq!(
        get(&server, &path, Some(&idle_live), &[]).await.status(),
        StatusCode::OK
    );
    clock.set(start, 31 * DAY);
    assert_dead(&get(&server, &path, Some(&idle_dead), &[]).await);

    // Used every 20 days, a session still ends 90 days after it began.
    let lasting = server.session_at(&user, start);
    for day in [20, 40, 60, 80, 89] {
        clock.set(start, day * DAY);
        assert_eq!(
            get(&server, &path, Some(&lasting), &[]).await.status(),
            StatusCode::OK,
            "day {day}"
        );
    }
    clock.set(start, 91 * DAY);
    assert_dead(&get(&server, &path, Some(&lasting), &[]).await);

    // The lock itself is enforced at every use, not only by the sessions a lock deletes.
    let locked = server.session_at(&user, clock.now());
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");
    assert_dead(&get(&server, &path, Some(&locked), &[]).await);
}

// On a path open to the public, a dead cookie reads as signed out: a public project still draws
// and a private one is hidden, while the same cookie on a path that needs a credential is refused.
#[tokio::test]
async fn dead_session_cookies_read_as_anonymous_on_public_paths() {
    let clock = TestClock::new();
    let server = server_with_clock(&clock).await;
    let user = server.signup("Stale User", "stale-user@example.com").await;
    let locked_user = server
        .signup("Locked User", "locked-user@example.com")
        .await;
    let org = server.create_org(&user, "Stale Org").await;
    let public = server.create_project(&user, &org, "Stale Public").await;
    let private = server.create_project(&user, &org, "Stale Private").await;
    make_private(&server, private.uuid);
    let start = clock.now();

    let idle = server.session_at(&user, start);
    let absolute = server.session_at(&user, start);
    diesel::update(
        schema::session::table
            .filter(schema::session::token_hash.eq(SessionTokenHash::from(&absolute))),
    )
    .set(schema::session::last_used.eq(start.timestamp() + 89 * DAY))
    .execute(&mut server.db_conn())
    .expect("Failed to use the session");
    clock.set(start, 91 * DAY);
    let locked = server.session(&locked_user);
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(locked_user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");
    let unknown = SessionToken::generate().expect("Failed to generate a token");
    let live = server.session(&user);

    let public_path = format!("/v0/projects/{}", public.slug);
    let private_path = format!("/v0/projects/{}", private.slug);
    for (dead, owner) in [
        (&idle, &user),
        (&absolute, &user),
        (&locked, &locked_user),
        (&unknown, &user),
    ] {
        let resp = get(&server, &public_path, Some(dead), &[]).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let project: JsonProject = resp.json().await.expect("Failed to parse project");
        assert_eq!(project.uuid, public.uuid);
        assert_eq!(
            get(&server, &private_path, Some(dead), &[]).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_dead(&get(&server, &user_path(owner), Some(dead), &[]).await);
    }
    assert_eq!(
        get(&server, &private_path, Some(&live), &[]).await.status(),
        StatusCode::OK
    );
}

// A use renews the session at most once an hour.
#[tokio::test]
async fn session_renews_at_most_once_an_hour() {
    let clock = TestClock::new();
    let server = server_with_clock(&clock).await;
    let user = server.signup("Renew User", "renew-user@example.com").await;
    let start = clock.now();
    let token = server.session_at(&user, start);

    for (elapsed, last_used) in [
        (59 * MINUTE, 0),
        (61 * MINUTE, 61 * MINUTE),
        (61 * MINUTE + 59 * MINUTE, 61 * MINUTE),
        (61 * MINUTE + 61 * MINUTE, 122 * MINUTE),
    ] {
        clock.set(start, elapsed);
        let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            session_row(&server, &token).last_used.timestamp(),
            start.timestamp() + last_used,
            "after {elapsed} seconds"
        );
    }
}

// A renewal that cannot write never holds up or fails the read, and a later use renews instead.
#[tokio::test]
async fn renewal_never_fails_a_read() {
    let clock = TestClock::new();
    let server = server_with_clock(&clock).await;
    let user = server.signup("Busy User", "busy-user@example.com").await;
    let start = clock.now();
    let token = server.session_at(&user, start);
    clock.set(start, 2 * HOUR);

    {
        let _writer = server.context().database.connection.lock().await;
        let resp = tokio::time::timeout(
            Duration::from_secs(10),
            get(&server, &user_path(&user), Some(&token), &[]),
        )
        .await
        .expect("The read waited on the writer");
        assert_eq!(resp.status(), StatusCode::OK);
    }
    assert_eq!(session_row(&server, &token).last_used, start);

    server
        .db_conn()
        .batch_execute(
            "CREATE TRIGGER no_renewal BEFORE UPDATE ON session BEGIN SELECT RAISE(ABORT, 'no renewal'); END",
        )
        .expect("Failed to block renewals");
    let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(session_row(&server, &token).last_used, start);

    server
        .db_conn()
        .batch_execute("DROP TRIGGER no_renewal")
        .expect("Failed to allow renewals");
    let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        session_row(&server, &token).last_used.timestamp(),
        start.timestamp() + 2 * HOUR
    );
}

// A write by a session must come from the console's own origin; a read, or a write by a bearer
// credential, need not. A refused write does nothing.
#[tokio::test]
async fn session_writes_need_the_console_origin() {
    let server = TestServer::new().await;
    let user = server
        .signup("Origin User", "origin-user@example.com")
        .await;
    let token = server.session(&user);

    for (case, (headers, status)) in [
        (vec![("Sec-Fetch-Site", "same-site")], StatusCode::FORBIDDEN),
        (
            vec![("Sec-Fetch-Site", "cross-site")],
            StatusCode::FORBIDDEN,
        ),
        (vec![("Sec-Fetch-Site", "none")], StatusCode::FORBIDDEN),
        (
            vec![("Sec-Fetch-Site", "cross-site"), ("Origin", CONSOLE_ORIGIN)],
            StatusCode::FORBIDDEN,
        ),
        (
            vec![("Origin", "https://bencher.dev.example")],
            StatusCode::FORBIDDEN,
        ),
        (
            vec![("Origin", "http://localhost:6610")],
            StatusCode::FORBIDDEN,
        ),
        (vec![], StatusCode::FORBIDDEN),
        (vec![("Sec-Fetch-Site", "same-origin")], StatusCode::OK),
        (vec![("Origin", CONSOLE_ORIGIN)], StatusCode::OK),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("Origin User {case}");
        let resp = rename(&server, &user, Some(&token), &headers, &name).await;
        assert_eq!(resp.status(), status, "{headers:?}");
        let json_user: JsonUser = get(&server, &user_path(&user), Some(&token), &[])
            .await
            .json()
            .await
            .expect("Failed to parse user");
        assert_eq!(
            json_user.name.as_ref() == name,
            status == StatusCode::OK,
            "{headers:?}"
        );
    }

    let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = server
        .client
        .patch(server.api_url(&user_path(&user)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({"name": "Bearer Name"}))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

// Every write method is held to the console's origin: a POST from another site mints no key,
// and a PUT from another site is refused before its threshold is looked up.
#[tokio::test]
async fn cross_site_posts_and_puts_are_refused() {
    let server = TestServer::new().await;
    let user = server.signup("Post User", "post-user@example.com").await;
    let org = server.create_org(&user, "Post Org").await;
    let project = server.create_project(&user, &org, "Post Project").await;
    let token = server.session(&user);
    let cross_site = [("Sec-Fetch-Site", "cross-site")];

    let keys_path = format!("/v0/users/{}/keys", user.slug);
    let key = json!({"name": "cross-site-key"});
    let resp = send(&server, Method::POST, &keys_path, &token, &cross_site, &key).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(user_key_count(&server), 0);
    let resp = send(
        &server,
        Method::POST,
        &keys_path,
        &token,
        &[same_origin()],
        &key,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(user_key_count(&server), 1);

    let threshold_path = format!(
        "/v0/projects/{}/thresholds/{}",
        project.slug,
        ThresholdUuid::new()
    );
    let model = json!({"test": "percentage", "max_sample_size": 32, "upper_boundary": 0.05});
    let resp = send(
        &server,
        Method::PUT,
        &threshold_path,
        &token,
        &cross_site,
        &model,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp = send(
        &server,
        Method::PUT,
        &threshold_path,
        &token,
        &[same_origin()],
        &model,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// An `Authorization` header alone decides: a valid cookie never rescues a bad bearer,
// and never stands in for the bearer's own user.
#[tokio::test]
async fn bearer_decides_over_cookie() {
    let server = TestServer::new().await;
    let user = server.signup("Owner User", "owner-user@example.com").await;
    let other = server.signup("Other User", "other-user@example.com").await;
    let org = server.create_org(&user, "Owner Org").await;
    let project = server.create_project(&user, &org, "Owner Project").await;
    make_private(&server, project.uuid);
    let token = server.session(&user);
    let bad_bearer = [(bencher_json::AUTHORIZATION, "Bearer not-a-credential")];

    let resp = get(&server, &user_path(&user), Some(&token), &bad_bearer).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let project_path = format!("/v0/projects/{}", project.slug);
    let resp = get(&server, &project_path, Some(&token), &bad_bearer).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let other_bearer = bencher_json::bearer_header(&other.token);
    let other_bearer = [(bencher_json::AUTHORIZATION, other_bearer.as_str())];
    let resp = get(&server, &user_path(&user), Some(&token), &other_bearer).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp = get(&server, &project_path, Some(&token), &other_bearer).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// With a relay secret set, a cookie is honored only from a request that carries the secret.
#[tokio::test]
async fn relay_secret_gates_the_cookie() {
    let server = server_with_relay_secret(None).await;
    let user = server.signup("Relay User", "relay-user@example.com").await;
    let org = server.create_org(&user, "Relay Org").await;
    let project = server.create_project(&user, &org, "Relay Project").await;
    make_private(&server, project.uuid);
    let token = server.session(&user);
    let path = user_path(&user);
    let project_path = format!("/v0/projects/{}", project.slug);

    for secret in [None, Some("not-the-relay-secret")] {
        let headers: Vec<_> = secret
            .map(|secret| (bencher_json::RELAY_SECRET, secret))
            .into_iter()
            .collect();
        let resp = get(&server, &path, Some(&token), &headers).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{secret:?}");
        assert!(
            cleared_cookie(resp.headers()).is_none(),
            "an ignored cookie is not cleared: {secret:?}"
        );
        let resp = get(&server, &project_path, Some(&token), &headers).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{secret:?}");
    }

    let headers = [(bencher_json::RELAY_SECRET, RELAY_SECRET)];
    let resp = get(&server, &path, Some(&token), &headers).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = get(&server, &project_path, Some(&token), &headers).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// With a relay secret set, the relay's forwarded IP address keys every per-IP limit, but only on
// a request that carries the secret.
#[tokio::test]
async fn relay_secret_gates_the_forwarded_ip() {
    let server =
        server_with_relay_secret(Some(json!({"public": {"requests": {"minute": 1}}}))).await;
    let user = server
        .signup("Relay IP User", "relay-ip-user@example.com")
        .await;
    let org = server.create_org(&user, "Relay IP Org").await;
    let project = server.create_project(&user, &org, "Relay IP Project").await;
    let path = format!("/v0/projects/{}", project.slug);

    // The right secret: each reader is charged alone, whatever the edge address.
    for (reader, status) in [
        ("203.0.113.1", StatusCode::OK),
        ("203.0.113.2", StatusCode::OK),
        ("203.0.113.1", StatusCode::TOO_MANY_REQUESTS),
    ] {
        let headers = [
            ("X-Forwarded-For", "198.51.100.1"),
            (bencher_json::RELAY_CLIENT_IP, reader),
            (bencher_json::RELAY_SECRET, RELAY_SECRET),
        ];
        assert_eq!(
            get(&server, &path, None, &headers).await.status(),
            status,
            "{reader}"
        );
    }

    // No secret, or the wrong one: the forwarded address is ignored, and the edge is charged.
    for (edge, secret) in [
        ("198.51.100.2", None),
        ("198.51.100.3", Some("not-the-relay-secret")),
    ] {
        for (reader, status) in [
            ("203.0.113.3", StatusCode::OK),
            ("203.0.113.4", StatusCode::TOO_MANY_REQUESTS),
        ] {
            let mut headers = vec![
                ("X-Forwarded-For", edge),
                (bencher_json::RELAY_CLIENT_IP, reader),
            ];
            headers.extend(secret.map(|secret| (bencher_json::RELAY_SECRET, secret)));
            assert_eq!(
                get(&server, &path, None, &headers).await.status(),
                status,
                "{edge} {secret:?} {reader}"
            );
        }
    }

    // The right secret beside two forwarded addresses: neither is trusted, and the edge is charged.
    for (readers, status) in [
        (["203.0.113.5", "203.0.113.6"], StatusCode::OK),
        (
            ["203.0.113.7", "203.0.113.8"],
            StatusCode::TOO_MANY_REQUESTS,
        ),
    ] {
        let headers = [
            ("X-Forwarded-For", "198.51.100.4"),
            (bencher_json::RELAY_CLIENT_IP, readers[0]),
            (bencher_json::RELAY_CLIENT_IP, readers[1]),
            (bencher_json::RELAY_SECRET, RELAY_SECRET),
        ];
        assert_eq!(
            get(&server, &path, None, &headers).await.status(),
            status,
            "{readers:?}"
        );
    }
}

// With no relay secret set, the cookie is honored from any caller, and no forwarded IP address
// is ever trusted.
#[tokio::test]
async fn no_relay_secret_honors_the_cookie_and_ignores_the_forwarded_ip() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 1}}})).await;
    let user = server
        .signup("Direct User", "direct-user@example.com")
        .await;
    let org = server.create_org(&user, "Direct Org").await;
    let project = server.create_project(&user, &org, "Direct Project").await;
    let token = server.session(&user);

    let resp = get(&server, &user_path(&user), Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let path = format!("/v0/projects/{}", project.slug);
    for (reader, status) in [
        ("203.0.113.5", StatusCode::OK),
        ("203.0.113.6", StatusCode::TOO_MANY_REQUESTS),
    ] {
        let headers = [
            ("X-Forwarded-For", "198.51.100.4"),
            (bencher_json::RELAY_CLIENT_IP, reader),
            (bencher_json::RELAY_SECRET, RELAY_SECRET),
        ];
        assert_eq!(
            get(&server, &path, None, &headers).await.status(),
            status,
            "{reader}"
        );
    }
}

// Reading the session names its user and its life, and never its secret, which the database
// holds only as a hash.
#[tokio::test]
async fn get_session_never_carries_the_session_token() {
    let clock = TestClock::new();
    let server = server_with_clock(&clock).await;
    let user = server
        .signup("Reader User", "reader-user@example.com")
        .await;
    let start = clock.now();
    let token = server.session_at(&user, start);

    let resp = get(&server, "/v0/auth/session", Some(&token), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_no_token(resp.headers(), &token);
    let body = resp.text().await.expect("Failed to read the session");
    assert!(!body.contains(token.as_ref()), "{body}");
    let session: JsonSession = serde_json::from_str(&body).expect("Failed to parse the session");
    assert_eq!(session.user.uuid, user.uuid);
    assert_eq!(session.method, SessionMethod::Email);
    assert_eq!(session.created.timestamp(), start.timestamp());
    assert_eq!(session.expiration.timestamp(), start.timestamp() + 30 * DAY);

    let row = session_row(&server, &token);
    assert_eq!(row.token_hash, SessionTokenHash::from(&token));
    let row_text = format!("{row:?}");
    assert!(!row_text.contains(token.as_ref()), "{row_text}");
}

// Reading the session needs a browser session, not a bearer credential.
#[tokio::test]
async fn get_session_needs_a_session_cookie() {
    let server = TestServer::new().await;
    let user = server
        .signup("Bearer Reader", "bearer-reader@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// Ending the session deletes it and clears the cookie, from the console's own origin only.
#[tokio::test]
async fn delete_session_ends_it() {
    let server = TestServer::new().await;
    let user = server
        .signup("Leaving User", "leaving-user@example.com")
        .await;
    let token = server.session(&user);
    let other = server.session(&user);

    let resp = delete_session(&server, &token, &[("Sec-Fetch-Site", "cross-site")]).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        get(&server, "/v0/auth/session", Some(&token), &[])
            .await
            .status(),
        StatusCode::OK
    );

    let resp = delete_session(&server, &token, &[same_origin()]).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_no_token(resp.headers(), &token);
    assert_eq!(
        cleared_cookie(resp.headers()).as_deref(),
        Some(
            SessionCookie::Host
                .clear()
                .to_str()
                .expect("Invalid header")
        )
    );
    assert_dead(&get(&server, "/v0/auth/session", Some(&token), &[]).await);

    // The reader's other session lives on.
    assert_eq!(
        get(&server, "/v0/auth/session", Some(&other), &[])
            .await
            .status(),
        StatusCode::OK
    );
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

async fn server_with_clock(clock: &TestClock) -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await
}

async fn server_with_relay_secret(rate_limiting: Option<serde_json::Value>) -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        relay_secret: Some(RELAY_SECRET.parse::<Secret>().expect("Invalid secret")),
        rate_limiting: rate_limiting.map(rate_limiting_from),
        ..TestServerOptions::default()
    })
    .await
}

async fn rate_limited_server(rate_limiting: serde_json::Value) -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        rate_limiting: Some(rate_limiting_from(rate_limiting)),
        ..TestServerOptions::default()
    })
    .await
}

fn rate_limiting_from(rate_limiting: serde_json::Value) -> RateLimiting {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(rate_limiting).expect("Invalid rate limiting");
    RateLimiting::from(rate_limiting)
}

fn user_path(user: &TestUser) -> String {
    format!("/v0/users/{}", user.slug)
}

fn same_origin() -> (&'static str, &'static str) {
    ("Sec-Fetch-Site", "same-origin")
}

async fn get(
    server: &TestServer,
    path: &str,
    token: Option<&SessionToken>,
    headers: &[(&str, &str)],
) -> reqwest::Response {
    let mut request = server.client.get(server.api_url(path));
    if let Some(token) = token {
        request = request.header(header::COOKIE, session_cookie(token));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.send().await.expect("Request failed")
}

async fn rename(
    server: &TestServer,
    user: &TestUser,
    token: Option<&SessionToken>,
    headers: &[(&str, &str)],
    name: &str,
) -> reqwest::Response {
    let mut request = server
        .client
        .patch(server.api_url(&user_path(user)))
        .json(&json!({"name": name}));
    if let Some(token) = token {
        request = request.header(header::COOKIE, session_cookie(token));
    }
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    request.send().await.expect("Request failed")
}

async fn send(
    server: &TestServer,
    method: Method,
    path: &str,
    token: &SessionToken,
    headers: &[(&str, &str)],
    body: &serde_json::Value,
) -> reqwest::Response {
    let mut request = server
        .client
        .request(method, server.api_url(path))
        .header(header::COOKIE, session_cookie(token))
        .json(body);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.send().await.expect("Request failed")
}

async fn delete_session(
    server: &TestServer,
    token: &SessionToken,
    headers: &[(&str, &str)],
) -> reqwest::Response {
    let mut request = server
        .client
        .delete(server.api_url("/v0/auth/session"))
        .header(header::COOKIE, session_cookie(token));
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.send().await.expect("Request failed")
}

fn assert_dead(resp: &reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        cleared_cookie(resp.headers()).as_deref(),
        Some(
            SessionCookie::Host
                .clear()
                .to_str()
                .expect("Invalid header")
        )
    );
}

fn cleared_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::SET_COOKIE)
        .map(|value| value.to_str().expect("Invalid header").to_owned())
}

fn assert_no_token(headers: &HeaderMap, token: &SessionToken) {
    for (name, value) in headers {
        let value = String::from_utf8_lossy(value.as_bytes());
        assert!(!value.contains(token.as_ref()), "{name}: {value}");
    }
}

fn session_row(server: &TestServer, token: &SessionToken) -> QuerySession {
    schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(token)))
        .first::<QuerySession>(&mut server.db_conn())
        .expect("Failed to find the session")
}

fn user_key_count(server: &TestServer) -> i64 {
    schema::user_key::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count user keys")
}

fn make_private(server: &TestServer, project: bencher_json::ProjectUuid) {
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(project)))
        .set(schema::project::visibility.eq(bencher_json::project::Visibility::Private))
        .execute(&mut server.db_conn())
        .expect("Failed to make the project private");
}
