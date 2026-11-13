#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for reading and confirming an email change by the secret its link carries.

#[cfg(feature = "plus")]
use bencher_api_tests::TestServerOptions;
use bencher_api_tests::{TestServer, TestUser, helpers::seed_token, session_cookie};
use bencher_json::{
    DateTime, EmailChangeSecret, JsonEmailChange, SessionToken, SessionTokenHash, TokenUuid,
    UserKey,
};
#[cfg(feature = "plus")]
use bencher_json::{
    RELAY_CLIENT_IP, RELAY_SECRET as RELAY_SECRET_HEADER, system::config::JsonRateLimiting,
};
#[cfg(feature = "plus")]
use bencher_schema::context::RateLimiting;
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::{Value, json};

const ADA: &str = "ada@example.com";
const NEW: &str = "ada@lovelace.dev";
const MINUTE: i64 = 60;
const SAME_ORIGIN: (&str, &str) = ("Sec-Fetch-Site", "same-origin");
const CROSS_SITE: (&str, &str) = ("Sec-Fetch-Site", "cross-site");
const READ: &str = "/v0/auth/email/change";
const CONFIRM: &str = "/v0/auth/email/change/confirm";

// The read names the user's email and the new one to anyone with the secret, and nothing else.
#[tokio::test]
async fn read_names_both_addresses_while_it_waits() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let secret = server.email_change(&ada, Some(&server.session(&ada)), NEW);

    let (status, json) = post(&server, READ, &secret, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let change: JsonEmailChange = serde_json::from_value(json.clone()).expect("Invalid change");
    assert_eq!(
        (change.email.as_ref(), change.new_email.as_ref()),
        (ADA, NEW)
    );
    assert!(!json.to_string().contains(secret.as_ref()));
    assert_eq!(email_of(&server, &ada), ADA);
}

// A change no row holds, one that expired, and one asked for again are one refusal, which tells
// none of them apart; a confirmed change reads as used until it expires, then as expired.
// Kills a read that tells an unknown secret from a dead one, or a used change from either.
#[tokio::test]
async fn read_refuses_unknown_expired_and_renewed_alike_and_a_used_one_as_used() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let alan = server.signup("Alan", "alan@example.com").await;
    let unknown = EmailChangeSecret::generate().expect("Failed to make a secret");
    let expired = server.email_change_at(&ada, None, NEW, minutes_ago(30));
    let renewed = server.email_change(&grace, None, "grace@hopper.dev");
    let _renewal = server.email_change(&grace, None, "grace@hopper.dev");
    let used = server.email_change(&alan, None, "alan@turing.dev");
    assert_eq!(
        post(&server, CONFIRM, &used, None, SAME_ORIGIN).await.0,
        StatusCode::OK
    );

    let refusals: Vec<(StatusCode, Value)> = [
        post(&server, READ, &unknown, None, SAME_ORIGIN).await,
        post(&server, READ, &expired, None, SAME_ORIGIN).await,
        post(&server, READ, &renewed, None, SAME_ORIGIN).await,
    ]
    .into_iter()
    .map(|(status, mut json)| {
        // Each error carries its own request's ID.
        json["request_id"] = Value::Null;
        (status, json)
    })
    .collect();
    let first = refusals.first().expect("No refusal").clone();
    assert_eq!(first.0, StatusCode::BAD_REQUEST);
    assert_eq!(first.1["error_code"], "email_change_expired");
    assert!(
        refusals.iter().all(|refusal| *refusal == first),
        "{refusals:?}"
    );

    let (status, json) = post(&server, READ, &used, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error_code"], "email_change_used");
}

// A confirmed change reads as used only until it expires, and from then on as expired, both to a
// read and to a confirm.
#[tokio::test]
async fn used_change_reads_as_expired_once_it_expires() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let secret = server.email_change(&ada, Some(&server.session(&ada)), NEW);
    assert_eq!(
        post(&server, CONFIRM, &secret, None, SAME_ORIGIN).await.0,
        StatusCode::OK
    );
    diesel::update(schema::email_change::table)
        .set(schema::email_change::expiration.eq(minutes_ago(1).timestamp()))
        .execute(&mut server.db_conn())
        .expect("Failed to age the change");

    for path in [READ, CONFIRM] {
        let (status, json) = post(&server, path, &secret, None, SAME_ORIGIN).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {json}");
        assert_eq!(json["error_code"], "email_change_expired", "{path}");
    }
    assert_eq!(email_of(&server, &ada), NEW);
}

// Both endpoints serve the console alone, and a confirm from another site changes nothing.
#[tokio::test]
async fn read_and_confirm_need_the_console_origin() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let session = server.session(&ada);
    let secret = server.email_change(&ada, Some(&session), NEW);

    for path in [READ, CONFIRM] {
        for session in [None, Some(&session)] {
            let (status, json) = post(&server, path, &secret, session, CROSS_SITE).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
            assert_eq!(json["error_code"], "cross_origin", "{path}");
        }
    }
    assert_eq!(email_of(&server, &ada), ADA);
    assert_eq!(confirmed(&server, &ada), None);
}

// Both endpoints are charged to the reader's address, as starting a sign-in is, so secrets cannot
// be guessed faster than sign-ins can start.
#[cfg(feature = "plus")]
#[tokio::test]
async fn read_and_confirm_are_charged_to_the_readers_address() {
    const RELAY_SECRET: &str = "a-relay-secret-for-tests";
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"attempts": {"minute": 2}}}))
            .expect("Invalid rate limiting");
    let server = TestServer::new_with_options(TestServerOptions {
        relay_secret: Some(RELAY_SECRET.parse().expect("Invalid secret")),
        rate_limiting: Some(RateLimiting::from(rate_limiting)),
        ..TestServerOptions::default()
    })
    .await;
    let unknown = EmailChangeSecret::generate().expect("Failed to make a secret");
    let relayed = |path: &str, ip: &'static str| {
        server
            .client
            .post(server.api_url(path))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .header(RELAY_SECRET_HEADER, RELAY_SECRET)
            .header(RELAY_CLIENT_IP, ip)
            .json(&json!({"secret": unknown.as_ref()}))
    };
    for path in [READ, CONFIRM] {
        let resp = relayed(path, "203.0.113.7")
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{path}");
    }
    for path in [READ, CONFIRM] {
        let resp = relayed(path, "203.0.113.7")
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
        let json: Value = resp.json().await.expect("Invalid body");
        assert_eq!(json["error_code"], "too_many_attempts", "{path}");
    }
    let resp = relayed(CONFIRM, "203.0.113.8")
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// Confirming needs no credential and works the same wherever the link opens: signed out, in the
// session that asked, in another session of the user, or in another user's. Each time the email
// switches, proven, the legacy tokens are revoked, the change reads as used, every session of
// the user ends but the one that asked, and keys stay.
// Kills a confirm that keeps the session it opens in, ends the asking session or another user's,
// leaves a legacy token, ends a key, or needs a credential.
#[tokio::test]
async fn confirm_signed_out_switches_once_and_keeps_only_the_asking_session() {
    confirm_switches_once_and_keeps_only_the_asking_session(OpenedIn::SignedOut).await;
}

#[tokio::test]
async fn confirm_in_the_asking_session_switches_once_and_keeps_only_it() {
    confirm_switches_once_and_keeps_only_the_asking_session(OpenedIn::Asking).await;
}

#[tokio::test]
async fn confirm_in_another_session_switches_once_and_keeps_only_the_asking_session() {
    confirm_switches_once_and_keeps_only_the_asking_session(OpenedIn::Other).await;
}

#[tokio::test]
async fn confirm_in_another_users_session_switches_once_and_keeps_only_the_asking_session() {
    confirm_switches_once_and_keeps_only_the_asking_session(OpenedIn::AnotherUser).await;
}

#[derive(Debug, Clone, Copy)]
enum OpenedIn {
    SignedOut,
    Asking,
    Other,
    AnotherUser,
}

async fn confirm_switches_once_and_keeps_only_the_asking_session(opened_in: OpenedIn) {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let key = server.create_user_key(&ada, "Ada Key").await;
    let api_token = seed_token(&server, &ada, "Legacy Token");
    let asking = server.session(&ada);
    let other = server.session(&ada);
    let graces = server.session(&grace);
    let secret = server.email_change(&ada, Some(&asking), NEW);
    let cookie = match opened_in {
        OpenedIn::SignedOut => None,
        OpenedIn::Asking => Some(&asking),
        OpenedIn::Other => Some(&other),
        OpenedIn::AnotherUser => Some(&graces),
    };

    let (status, json) = post(&server, CONFIRM, &secret, cookie, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let change: JsonEmailChange = serde_json::from_value(json).expect("Invalid change");
    assert_eq!(change.new_email.as_ref(), NEW);
    assert_eq!(email_of(&server, &ada), NEW);
    assert!(email_verified_of(&server, &ada).is_some());
    assert!(revoked(&server, api_token.uuid).is_some());
    assert!(confirmed(&server, &ada).is_some());
    assert!(session_exists(&server, &asking));
    assert!(!session_exists(&server, &other));
    assert!(session_exists(&server, &graces));
    assert_eq!(view_status(&server, &key, &ada).await, StatusCode::OK);

    let (status, json) = post(&server, CONFIRM, &secret, cookie, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        json.get("error_code").and_then(Value::as_str),
        Some("email_change_used")
    );
}

// Confirms sent together switch the email once: one confirms and every other reads it as used.
#[tokio::test]
async fn parallel_confirms_switch_once() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let asking = server.session(&ada);
    let secret = server.email_change(&ada, Some(&asking), NEW);

    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let request = server
            .client
            .post(server.api_url(CONFIRM))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .json(&json!({"secret": secret.as_ref()}));
        requests.spawn(async move {
            let resp = request.send().await.expect("Request failed");
            let status = resp.status();
            let json: Value = resp.json().await.unwrap_or(Value::Null);
            (status, json)
        });
    }
    let answers = requests.join_all().await;
    let confirmed_count = answers
        .iter()
        .filter(|(status, _)| *status == StatusCode::OK)
        .count();
    assert_eq!(confirmed_count, 1, "{answers:?}");
    assert!(
        answers
            .iter()
            .filter(|(status, _)| *status != StatusCode::OK)
            .all(|(status, json)| *status == StatusCode::BAD_REQUEST
                && json["error_code"] == "email_change_used"),
        "{answers:?}"
    );
    assert_eq!(email_of(&server, &ada), NEW);
    assert!(session_exists(&server, &asking));
}

// An expired change, a renewed change's earlier link, and a change whose user's email moved since
// the ask each refuse as expired and change nothing.
#[tokio::test]
async fn confirm_of_an_expired_change_refuses_and_changes_nothing() {
    confirm_refuses_as_expired_and_changes_nothing(Refused::Expired).await;
}

#[tokio::test]
async fn confirm_of_a_renewed_changes_earlier_link_refuses_and_changes_nothing() {
    confirm_refuses_as_expired_and_changes_nothing(Refused::Renewed).await;
}

// A server admin's change ends the pending change, so the email moves under one only by a write
// that skips the API, which the confirm still refuses.
#[tokio::test]
async fn confirm_after_the_email_moved_refuses_and_changes_nothing() {
    confirm_refuses_as_expired_and_changes_nothing(Refused::Moved).await;
}

#[derive(Debug, Clone, Copy)]
enum Refused {
    Expired,
    Renewed,
    Moved,
}

async fn confirm_refuses_as_expired_and_changes_nothing(refused: Refused) {
    const MOVED: &str = "ada@moved.example";
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let api_token = seed_token(&server, &ada, "Legacy Token");
    let asking = server.session(&ada);
    let other = server.session(&ada);
    let secret = match refused {
        Refused::Expired => server.email_change_at(&ada, Some(&asking), NEW, minutes_ago(30)),
        Refused::Renewed => {
            let first = server.email_change(&ada, Some(&asking), NEW);
            let _renewed = server.email_change(&ada, Some(&asking), "ada@babbage.dev");
            first
        },
        Refused::Moved => {
            let secret = server.email_change(&ada, Some(&asking), NEW);
            diesel::update(schema::user::table.filter(schema::user::uuid.eq(ada.uuid)))
                .set(schema::user::email.eq(MOVED))
                .execute(&mut server.db_conn())
                .expect("Failed to move the email");
            secret
        },
    };
    let before = email_of(&server, &ada);

    let (status, json) = post(&server, CONFIRM, &secret, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(
        json.get("error_code").and_then(Value::as_str),
        Some("email_change_expired")
    );
    assert_eq!(email_of(&server, &ada), before);
    assert_eq!(confirmed(&server, &ada), None);
    assert!(session_exists(&server, &other));
    assert_eq!(revoked(&server, api_token.uuid), None);
}

// A new email another user signed up with since the ask is a conflict that changes nothing.
#[tokio::test]
async fn confirm_of_a_taken_email_conflicts_and_changes_nothing() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let api_token = seed_token(&server, &ada, "Legacy Token");
    let asking = server.session(&ada);
    let other = server.session(&ada);
    let secret = server.email_change(&ada, Some(&asking), NEW);
    let _taken = server.signup("Other Ada", NEW).await;

    let (status, json) = post(&server, CONFIRM, &secret, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::CONFLICT, "{json}");
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("The email is already in use")),
        "{json}"
    );
    assert_eq!(email_of(&server, &ada), ADA);
    assert_eq!(confirmed(&server, &ada), None);
    assert_eq!(revoked(&server, api_token.uuid), None);
    assert!(session_exists(&server, &other));
}

// A server admin's lock ends the pending change, so a user is locked under one only by a write that
// skips the API, which the confirm refuses as a locked account's sign-in is, changing nothing.
#[tokio::test]
async fn confirm_refuses_a_locked_user() {
    let server = TestServer::new().await;
    let ada = server.signup("Ada", ADA).await;
    let secret = server.email_change(&ada, None, NEW);
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(ada.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");

    let (status, json) = post(&server, CONFIRM, &secret, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{json}");
    assert_eq!(json["error_code"], "locked");
    assert_eq!(email_of(&server, &ada), ADA);
    assert_eq!(confirmed(&server, &ada), None);
}

// A confirmed email joins the organization whose single sign-on domain it has, as an email proof
// at sign-in does, and the user's OAuth grants and provider pins stay.
#[cfg(feature = "plus")]
#[tokio::test]
async fn confirm_joins_single_sign_on_and_keeps_grants_and_pins() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let org = server.create_org(&admin, "Sso Org").await;
    server
        .license_org(&admin, &org, bencher_json::PlanLevel::Enterprise)
        .await;
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/sso", org.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&json!({ "domain": "sso.example.com" }))
        .send()
        .await
        .expect("Request failed");
    assert!(resp.status().is_success(), "{}", resp.status());
    let ada = server.signup("Ada", ADA).await;
    let grant = bencher_api_tests::helpers::seed_oauth_grant(
        &server,
        &ada,
        &"projects:read".parse().expect("Invalid scopes"),
        &bencher_api_tests::helpers::oauth_resource(),
        None,
        DateTime::now(),
    );
    let user_id: i32 = schema::user::table
        .filter(schema::user::uuid.eq(ada.uuid))
        .select(schema::user::id)
        .first(&mut server.db_conn())
        .expect("No user");
    diesel::insert_into(schema::provider_pin::table)
        .values((
            schema::provider_pin::user_id.eq(user_id),
            schema::provider_pin::provider.eq(0),
            schema::provider_pin::account_id.eq("12345"),
            schema::provider_pin::handle.eq("ada"),
            schema::provider_pin::created.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to pin");
    assert!(!bencher_api_tests::helpers::user_organizations(&server, ada.uuid).contains(&org.uuid));
    let secret = server.email_change(&ada, None, "ada@sso.example.com");

    let (status, json) = post(&server, CONFIRM, &secret, None, SAME_ORIGIN).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(bencher_api_tests::helpers::user_organizations(&server, ada.uuid).contains(&org.uuid));
    assert_eq!(
        schema::oauth_grant::table
            .filter(schema::oauth_grant::uuid.eq(grant.uuid))
            .count()
            .get_result::<i64>(&mut server.db_conn())
            .expect("Failed to count"),
        1
    );
    assert_eq!(
        schema::provider_pin::table
            .filter(schema::provider_pin::user_id.eq(user_id))
            .count()
            .get_result::<i64>(&mut server.db_conn())
            .expect("Failed to count"),
        1
    );
}

async fn post(
    server: &TestServer,
    path: &str,
    secret: &EmailChangeSecret,
    session: Option<&SessionToken>,
    origin: (&str, &str),
) -> (StatusCode, Value) {
    let mut request = server
        .client
        .post(server.api_url(path))
        .header(origin.0, origin.1)
        .json(&json!({"secret": secret.as_ref()}));
    if let Some(session) = session {
        request = request.header("Cookie", session_cookie(session));
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("No body");
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn minutes_ago(minutes: i64) -> DateTime {
    DateTime::try_from(DateTime::now().timestamp() - minutes * MINUTE).expect("Invalid time")
}

fn email_of(server: &TestServer, user: &TestUser) -> String {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::email)
        .first(&mut server.db_conn())
        .expect("No user")
}

fn email_verified_of(server: &TestServer, user: &TestUser) -> Option<i64> {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::email_verified)
        .first(&mut server.db_conn())
        .expect("No user")
}

fn confirmed(server: &TestServer, user: &TestUser) -> Option<i64> {
    schema::email_change::table
        .inner_join(schema::user::table)
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::email_change::confirmed)
        .first(&mut server.db_conn())
        .expect("No email change")
}

fn revoked(server: &TestServer, token: TokenUuid) -> Option<i64> {
    schema::token::table
        .filter(schema::token::uuid.eq(token))
        .select(schema::token::revoked)
        .first(&mut server.db_conn())
        .expect("No token")
}

fn session_exists(server: &TestServer, session: &SessionToken) -> bool {
    schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(session)))
        .select(schema::session::id)
        .first::<i32>(&mut server.db_conn())
        .optional()
        .expect("Failed to read sessions")
        .is_some()
}

async fn view_status(server: &TestServer, key: &UserKey, user: &TestUser) -> StatusCode {
    server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed")
        .status()
}
