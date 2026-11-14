#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for deprecated user API tokens over REST: a token is found by the hash of
//! its JWT, and its row alone decides whose it is, whether it is revoked, and when it expires.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use bencher_api_tests::{
    TestServer, TestServerOptions, TestUser,
    helpers::{seed_token, seed_token_jwt, token_last_used},
};
use bencher_json::{Clock, DateTime, JsonUser, Jwt, TokenUuid};
use bencher_schema::{error::INVALID_CREDENTIAL, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

// Kills a token resolved by the email its JWT names, a signature check, or a token that acts as
// anyone but the user its row belongs to.
#[tokio::test]
async fn legacy_token_acts_as_its_rows_user() {
    let server = TestServer::new().await;
    let _admin = server.signup("Root", "root-row@example.com").await;
    let owner = server.signup("Owner", "owner-row@example.com").await;
    let other = server.signup("Other", "other-row@example.com").await;

    let seeded = seed_token(&server, &owner, "Owner Token");
    let resp = get_user(&server, seeded.token.as_ref(), &owner).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // A JWT that names the other user's email and carries a signature that is not its own.
    let names_other = mint(&server, &other, 1);
    let unsigned = with_signature_of(&names_other, &mint(&server, &owner, 2));
    seed_token_jwt(&server, &owner, "Unsigned Token", &unsigned, far_future());

    let resp = get_user(&server, unsigned.as_ref(), &owner).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: JsonUser = resp.json().await.expect("Failed to parse user");
    assert_eq!(json.uuid, owner.uuid);
    let resp = get_user(&server, unsigned.as_ref(), &other).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// Kills a token accepted when revoked, past its row's expiration, held by a locked user, or with
// no row at all, whatever its signature.
#[tokio::test]
async fn dead_legacy_tokens_are_refused() {
    let server = TestServer::new().await;
    let user = server.signup("Dead User", "dead-legacy@example.com").await;
    let locked = server
        .signup("Locked User", "locked-legacy@example.com")
        .await;

    let revoked = seed_token(&server, &user, "Revoked Token");
    revoke(&server, revoked.uuid);

    // Its JWT is good for decades, but its row expired a second ago.
    let expired = mint(&server, &user, 1);
    let now = server.context().clock.now().timestamp();
    seed_token_jwt(&server, &user, "Expired", &expired, timestamp(now - 1));

    let locked_token = seed_token(&server, &locked, "Locked Token");
    lock(&server, &locked);

    let rowless = mint(&server, &user, 2);

    for (bearer, owner, case) in [
        (&revoked.token, &user, "revoked"),
        (&expired, &user, "expired"),
        (&locked_token.token, &locked, "locked"),
        (&rowless, &user, "rowless"),
    ] {
        let resp = get_user(&server, bearer.as_ref(), owner).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{case}");
        assert_eq!(message(resp).await, INVALID_CREDENTIAL, "{case}");
    }

    let live = seed_token(&server, &user, "Live Token");
    let resp = get_user(&server, live.token.as_ref(), &user).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// Kills a use recorded more than once an hour, or never.
#[tokio::test]
async fn legacy_token_use_is_recorded_at_most_once_an_hour() {
    const MINUTE: i64 = 60;
    let now = Arc::new(AtomicI64::new(DateTime::now().timestamp()));
    let clock = Arc::clone(&now);
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(move || {
            timestamp(clock.load(Ordering::SeqCst))
        }))),
        ..TestServerOptions::default()
    })
    .await;
    let user = server.signup("Used User", "used-legacy@example.com").await;
    let seeded = seed_token(&server, &user, "Used Token");
    assert_eq!(token_last_used(&server, seeded.uuid), None);

    let start = now.load(Ordering::SeqCst);
    for (elapsed, recorded) in [
        (0, 0),
        (59 * MINUTE, 0),
        (60 * MINUTE, 60 * MINUTE),
        (119 * MINUTE, 60 * MINUTE),
        (121 * MINUTE, 121 * MINUTE),
    ] {
        now.store(start + elapsed, Ordering::SeqCst);
        let resp = get_user(&server, seeded.token.as_ref(), &user).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            token_last_used(&server, seeded.uuid),
            Some(timestamp(start + recorded)),
            "{elapsed} seconds in"
        );
    }
}

// Kills a token that outlives its row's expiration by an instant.
#[tokio::test]
async fn legacy_token_expires_at_its_rows_expiration() {
    let now = timestamp(DateTime::now().timestamp());
    let server = TestServer::new_at(now).await;
    let user = server
        .signup("Expiring User", "expiring-legacy@example.com")
        .await;
    let later = mint(&server, &user, 1);
    seed_token_jwt(
        &server,
        &user,
        "Later",
        &later,
        timestamp(now.timestamp() + 1),
    );
    let now_token = mint(&server, &user, 2);
    seed_token_jwt(&server, &user, "Now", &now_token, now);

    let resp = get_user(&server, later.as_ref(), &user).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = get_user(&server, now_token.as_ref(), &user).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// Kills a list or view that carries any part of a token's JWT.
#[tokio::test]
async fn token_reads_carry_no_jwt() {
    let server = TestServer::new().await;
    let user = server.signup("Reader", "reader-legacy@example.com").await;
    let seeded = seed_token(&server, &user, "Read Me");
    let (header, claims) = seeded.token.as_ref().split_once('.').expect("JWT header");
    let (payload, signature) = claims.split_once('.').expect("JWT payload");

    let list = format!("/v0/users/{}/tokens", user.slug);
    let view = format!("{list}/{}", seeded.uuid);
    for path in [list, view] {
        let resp = server
            .client
            .get(server.api_url(&path))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        let body = resp.text().await.expect("Failed to read the body");
        assert!(body.contains(&seeded.uuid.to_string()), "{body}");
        for part in [header, payload, signature] {
            assert!(!body.contains(part), "{path}: {body}");
        }
        let json: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse");
        let rows = json.as_array().cloned().unwrap_or_else(|| vec![json]);
        for row in rows {
            assert!(row.get("token").is_none(), "{path}: {row}");
        }
    }
}

async fn get_user(server: &TestServer, bearer: &str, user: &TestUser) -> reqwest::Response {
    server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .send()
        .await
        .expect("Request failed")
}

async fn message(resp: reqwest::Response) -> String {
    let json: serde_json::Value = resp.json().await.expect("Failed to parse the error");
    json.get("message")
        .and_then(serde_json::Value::as_str)
        .expect("The error has no message")
        .to_owned()
}

/// A legacy token for `user`, made distinct from their others by `nonce`.
fn mint(server: &TestServer, user: &TestUser, nonce: u32) -> Jwt {
    server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX - nonce)
        .expect("Failed to mint a token")
}

/// `jwt` with the signature of `other`, so it is well formed but not signed.
fn with_signature_of(jwt: &Jwt, other: &Jwt) -> Jwt {
    let (message, _) = jwt.as_ref().rsplit_once('.').expect("JWT signature");
    let (_, signature) = other.as_ref().rsplit_once('.').expect("JWT signature");
    format!("{message}.{signature}")
        .parse()
        .expect("Invalid JWT")
}

fn far_future() -> DateTime {
    timestamp(DateTime::now().timestamp() + i64::from(u32::MAX))
}

fn timestamp(seconds: i64) -> DateTime {
    DateTime::try_from(seconds).expect("Invalid timestamp")
}

fn revoke(server: &TestServer, token: TokenUuid) {
    let rows = diesel::update(schema::token::table)
        .filter(schema::token::uuid.eq(token))
        .set(schema::token::revoked.eq(server.context().clock.now()))
        .execute(&mut server.db_conn())
        .expect("Failed to revoke the token");
    assert_eq!(rows, 1);
}

fn lock(server: &TestServer, user: &TestUser) {
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");
}
