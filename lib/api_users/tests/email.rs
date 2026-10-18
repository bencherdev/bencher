#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for changing a user's email, which a user confirms from the new email.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

#[cfg(feature = "plus")]
use bencher_api_tests::TestOrg;
use bencher_api_tests::{LogCapture, TestServer, TestUser, helpers::seed_token};
use bencher_json::{
    DateTime, Email, JsonConfirm, JsonTokens, JsonUser, Jwt, TokenUuid, UserUuid,
    system::auth::JsonAuthUser,
};
use bencher_schema::{AUTH_TOKEN_TTL, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

const SUBJECT: &str = "Subject: Confirm Bencher Email Change";
const LINK: &str = "Confirm Email: http://localhost:3000/auth/email?";
// 2100-01-01T00:00:00Z
const FROZEN_NOW: i64 = 4_102_444_800;

// Fails if a new email applies before it is confirmed, if no confirm link reaches the new email,
// if resending the current email sends one, or if asking revokes the user's legacy API tokens.
#[tokio::test]
async fn email_change_waits_for_confirm() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Wait User", "wait@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");

    patch_user(&server, &user.token, &user, &json_email(&user.email)).await;
    assert_eq!(
        confirm_links(&capture, &user.email).len(),
        0,
        "no confirm link"
    );

    let new_email = email("wait-new@example.com");
    let json = patch_user(&server, &user.token, &user, &json_email(&new_email)).await;
    assert_eq!(json.email, user.email);
    assert_eq!(
        view_user(&server, &user.token, &user).await.email,
        user.email
    );

    let links = confirm_links(&capture, &new_email);
    assert_eq!(links.len(), 1, "one confirm link to the new email");
    assert_eq!(links.first().map(|(uuid, _)| *uuid), Some(user.uuid));
    assert!(!is_revoked(&server, &user.token, &user, api_token.uuid).await);
}

// Fails if confirming does not switch the email, keeps the user's legacy API tokens,
// or answers with a session that is not for the new email.
#[tokio::test]
async fn email_change_confirm() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Confirm User", "confirm@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let new_email = email("confirm-new@example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let auth_user: JsonAuthUser = serde_json::from_str(&body).expect("Failed to parse auth user");
    assert_eq!(auth_user.user.email, new_email);

    let session = auth_user.token.to_string();
    assert_eq!(view_user(&server, &session, &user).await.email, new_email);
    assert!(is_revoked(&server, &session, &user, api_token.uuid).await);
}

// Fails if a confirm link works twice.
#[tokio::test]
async fn email_change_confirm_once() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin User", "once-admin@example.com").await;
    let user = server.signup("Once User", "once@example.com").await;
    let new_email = email("once-new@example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, &admin.token, &user).await.email,
        new_email
    );
}

// Fails if a confirm link outlives a login link, or expires by any clock but the server's.
#[tokio::test]
async fn email_change_confirm_expired() {
    let base = DateTime::TEST.timestamp();
    let time = Arc::new(AtomicI64::new(base));
    let clock_time = Arc::clone(&time);
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        DateTime::try_from(clock_time.load(Ordering::Relaxed)).expect("Invalid mocked timestamp")
    }));
    let capture = LogCapture::default();
    let server = TestServer::new_with_clock_and_log(clock, capture.logger()).await;
    let admin = server
        .signup("Admin User", "expired-admin@example.com")
        .await;
    let user = server.signup("Expired User", "expired@example.com").await;
    let new_email = email("expired-new@example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;
    let expiration = base + i64::from(AUTH_TOKEN_TTL);

    time.store(expiration + 1, Ordering::Relaxed);
    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        view_user(&server, &admin.token, &user).await.email,
        user.email
    );

    time.store(expiration, Ordering::Relaxed);
    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

// Fails if a confirm link changes the email of a user other than the one it names,
// whether opened at that other user's path or at the named user's path.
#[tokio::test]
async fn email_change_confirm_other_user() {
    let (server, _capture) = server_with_log().await;
    let admin = server.signup("Admin User", "other-admin@example.com").await;
    let user = server.signup("Named User", "named@example.com").await;
    let other = server.signup("Other User", "other@example.com").await;
    let token = server
        .token_key()
        .new_email_change(
            DateTime::now(),
            AUTH_TOKEN_TTL,
            user.uuid,
            stored_modified(&server, &other),
            other.email.clone(),
            email("other-new@example.com"),
        )
        .expect("Failed to mint email change token");

    let (status, body) = confirm(&server, other.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, &admin.token, &other).await.email,
        other.email
    );
    assert_eq!(
        view_user(&server, &admin.token, &user).await.email,
        user.email
    );
}

// Fails if a confirm link still works once the email it changes is no longer the user's.
#[tokio::test]
async fn email_change_confirm_superseded() {
    let (server, capture) = server_with_log().await;
    let user = server
        .signup("Superseded User", "superseded@example.com")
        .await;
    let first = email("superseded-first@example.com");
    let second = email("superseded-second@example.com");
    let first_token = request_change(&server, &capture, &user.token, &user, &first).await;
    let second_token = request_change(&server, &capture, &user.token, &user, &second).await;

    let (status, body) = confirm(&server, user.uuid, &second_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let auth_user: JsonAuthUser = serde_json::from_str(&body).expect("Failed to parse auth user");

    let (status, body) = confirm(&server, user.uuid, &first_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let session = auth_user.token.to_string();
    assert_eq!(view_user(&server, &session, &user).await.email, second);
}

// Fails if a link changes a user whose email is no longer the one it was sent for,
// as when another write lands between a request's read of the user and its own write.
#[tokio::test]
async fn email_change_confirm_other_email() {
    let (server, _capture) = server_with_log().await;
    let user = server.signup("Moved User", "moved@example.com").await;
    let token = current_link(
        &server,
        &user,
        email("moved-before@example.com"),
        email("moved-new@example.com"),
    );

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, &user.token, &user).await.email,
        user.email
    );
}

// Fails if a link works again once the user's email is back to the one it was sent for,
// or if a refused link revokes the user's legacy API tokens.
#[tokio::test]
async fn email_change_confirm_round_trip() {
    let (server, capture) = frozen_server_with_log().await;
    let user = server.signup("Round User", "round@example.com").await;
    let new_email = email("round-new@example.com");
    let first_token = request_change(&server, &capture, &user.token, &user, &new_email).await;
    let session = confirm_ok(&server, user.uuid, &first_token).await.token;
    let back_token = request_change(&server, &capture, session.as_ref(), &user, &user.email).await;
    let session = confirm_ok(&server, user.uuid, &back_token).await.token;
    let api_token = seed_token(&server, &user, "Legacy Token");

    let (status, body) = confirm(&server, user.uuid, &first_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, session.as_ref(), &user).await.email,
        user.email
    );
    assert!(!is_revoked(&server, session.as_ref(), &user, api_token.uuid).await);
}

// Fails if a link outlives a later change to the user, or if a refused link revokes the user's
// legacy API tokens.
#[tokio::test]
async fn email_change_confirm_after_edit() {
    let (server, capture) = frozen_server_with_log().await;
    let user = server.signup("Edit User", "edit@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let new_email = email("edit-new@example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;
    patch_user(
        &server,
        &user.token,
        &user,
        &serde_json::json!({ "name": "Edited User" }),
    )
    .await;

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, &user.token, &user).await.email,
        user.email
    );
    assert!(!is_revoked(&server, &user.token, &user, api_token.uuid).await);
}

// Fails if a confirmed email does not join the organization with its SSO domain, as a login does.
#[cfg(feature = "plus")]
#[tokio::test]
async fn email_change_confirm_joins_sso() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin User", "sso-admin@example.com").await;
    let org = server.create_org(&admin, "Sso Org").await;
    server
        .license_org(&admin, &org, bencher_json::PlanLevel::Enterprise)
        .await;
    add_sso(&server, &admin, &org, "sso.example.com").await;
    let user = server.signup("Sso User", "sso@example.com").await;
    assert_eq!(
        view_org(&server, &user.token, &org).await,
        StatusCode::NOT_FOUND
    );
    let new_email = email("sso@sso.example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;

    let session = confirm_ok(&server, user.uuid, &token).await.token;
    assert_eq!(
        view_org(&server, session.as_ref(), &org).await,
        StatusCode::OK
    );
}

// Fails if a confirm link takes an email another user signed up with after it was sent.
#[tokio::test]
async fn email_change_confirm_taken() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Taken User", "taken@example.com").await;
    let new_email = email("taken-new@example.com");
    let token = request_change(&server, &capture, &user.token, &user, &new_email).await;
    let other = server.signup("Other User", new_email.as_ref()).await;

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        view_user(&server, &user.token, &user).await.email,
        user.email
    );
    assert_eq!(
        view_user(&server, &other.token, &other).await.email,
        new_email
    );
}

// Fails if a confirm link changes the email of, or signs in, a locked user,
// as when a lock lands between a request's read of the user and its own write.
#[tokio::test]
async fn email_change_confirm_locked() {
    let (server, _capture) = server_with_log().await;
    let admin = server
        .signup("Admin User", "locked-admin@example.com")
        .await;
    let user = server.signup("Locked User", "locked@example.com").await;
    patch_user(
        &server,
        &admin.token,
        &user,
        &serde_json::json!({ "locked": true }),
    )
    .await;
    let token = current_link(
        &server,
        &user,
        user.email.clone(),
        email("locked-new@example.com"),
    );

    let (status, body) = confirm(&server, user.uuid, &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        view_user(&server, &admin.token, &user).await.email,
        user.email
    );
}

// Fails if a new email another user already has sends a confirm link or applies the other fields.
#[tokio::test]
async fn email_change_taken() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Asking User", "asking@example.com").await;
    let other = server.signup("Other User", "asked@example.com").await;

    let body = serde_json::json!({ "name": "Renamed User", "email": other.email });
    let (status, text) = send_patch(&server, &user.token, &user, &body).await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    assert_eq!(
        confirm_links(&capture, &other.email).len(),
        0,
        "no confirm link"
    );
    assert_eq!(view_user(&server, &user.token, &user).await.name, user.name);
}

// Fails if the other fields in a PATCH with a new email wait for the confirm too.
#[tokio::test]
async fn email_change_applies_other_fields() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Fields User", "fields@example.com").await;
    let new_email = email("fields-new@example.com");

    let body = serde_json::json!({
        "name": "Renamed User",
        "slug": "renamed-user",
        "email": new_email,
    });
    let json = patch_user(&server, &user.token, &user, &body).await;
    assert_eq!(json.name.as_ref(), "Renamed User");
    let slug: &str = json.slug.as_ref();
    assert_eq!(slug, "renamed-user");
    assert_eq!(json.email, user.email);
    assert_eq!(confirm_links(&capture, &new_email).len(), 1);
}

// Fails if a server admin's change to another user's email waits for a confirm,
// or keeps that user's legacy API tokens.
#[tokio::test]
async fn email_change_admin_other_user() {
    let (server, capture) = server_with_log().await;
    let admin = server
        .signup("Admin User", "direct-admin@example.com")
        .await;
    let user = server.signup("Direct User", "direct@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let new_email = email("direct-new@example.com");

    let json = patch_user(&server, &admin.token, &user, &json_email(&new_email)).await;
    assert_eq!(json.email, new_email);
    assert_eq!(
        confirm_links(&capture, &new_email).len(),
        0,
        "no confirm link"
    );

    let session = server
        .token_key()
        .new_client(new_email, u32::MAX)
        .expect("Failed to mint session")
        .to_string();
    assert!(is_revoked(&server, &session, &user, api_token.uuid).await);
}

// Fails if a server admin's change to their own email skips the confirm.
#[tokio::test]
async fn email_change_admin_self() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin User", "self-admin@example.com").await;
    let new_email = email("self-admin-new@example.com");

    let json = patch_user(&server, &admin.token, &admin, &json_email(&new_email)).await;
    assert_eq!(json.email, admin.email);
    assert_eq!(confirm_links(&capture, &new_email).len(), 1);
}

async fn server_with_log() -> (TestServer, LogCapture) {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    (server, capture)
}

/// A server whose clock stands still after the wall clock that stamps a signup,
/// so every write to a user lands in the same second.
async fn frozen_server_with_log() -> (TestServer, LogCapture) {
    let now = DateTime::try_from(FROZEN_NOW).expect("Invalid frozen timestamp");
    let capture = LogCapture::default();
    let server = TestServer::new_with_clock_and_log(
        bencher_json::Clock::Custom(Arc::new(move || now)),
        capture.logger(),
    )
    .await;
    (server, capture)
}

fn email(email: &str) -> Email {
    email.parse().expect("Invalid email")
}

fn json_email(email: &Email) -> serde_json::Value {
    serde_json::json!({ "email": email })
}

/// Ask for `new_email` with the user's session and return the token from the link it sends.
async fn request_change(
    server: &TestServer,
    capture: &LogCapture,
    bearer: &str,
    user: &TestUser,
    new_email: &Email,
) -> Jwt {
    patch_user(server, bearer, user, &json_email(new_email)).await;
    let (uuid, token) = confirm_links(capture, new_email)
        .pop()
        .expect("A confirm link was sent");
    assert_eq!(uuid, user.uuid);
    token
}

/// The user and token of every email change link sent to `email`, oldest first.
fn confirm_links(capture: &LogCapture, email: &Email) -> Vec<(UserUuid, Jwt)> {
    let to = format!("<{email}>");
    capture
        .lines()
        .iter()
        .filter(|line| line.contains(SUBJECT) && line.contains(&to))
        .filter_map(|line| {
            let query = line.split(LINK).nth(1)?.split_whitespace().next()?;
            let mut user = None;
            let mut token = None;
            for pair in query.split('&') {
                match pair.split_once('=')? {
                    ("user", value) => user = value.parse().ok(),
                    ("token", value) => token = value.parse().ok(),
                    _ => {},
                }
            }
            Some((user?, token?))
        })
        .collect()
}

/// Confirm with no credential, as a link opened on any device does.
async fn confirm(server: &TestServer, user: UserUuid, token: &Jwt) -> (StatusCode, String) {
    let resp = server
        .client
        .put(server.api_url(&format!("/v0/users/{user}/email")))
        .json(&JsonConfirm {
            token: token.clone(),
        })
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read response"))
}

async fn confirm_ok(server: &TestServer, user: UserUuid, token: &Jwt) -> JsonAuthUser {
    let (status, body) = confirm(server, user, token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).expect("Failed to parse auth user")
}

/// Patch the user with `bearer` and return the user it answers with.
async fn patch_user(
    server: &TestServer,
    bearer: &str,
    user: &TestUser,
    body: &serde_json::Value,
) -> JsonUser {
    let (status, text) = send_patch(server, bearer, user, body).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str(&text).expect("Failed to parse user")
}

async fn send_patch(
    server: &TestServer,
    bearer: &str,
    user: &TestUser,
    body: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read response"))
}

async fn view_user(server: &TestServer, bearer: &str, user: &TestUser) -> JsonUser {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("Failed to parse user")
}

/// A link bound to the user's current `modified`, as a request that wrote the user last mints.
fn current_link(server: &TestServer, user: &TestUser, email: Email, new_email: Email) -> Jwt {
    server
        .token_key()
        .new_email_change(
            DateTime::now(),
            AUTH_TOKEN_TTL,
            user.uuid,
            stored_modified(server, user),
            email,
            new_email,
        )
        .expect("Failed to mint email change token")
}

fn stored_modified(server: &TestServer, user: &TestUser) -> DateTime {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::modified)
        .first(&mut server.db_conn())
        .expect("Failed to get the user's modified time")
}

async fn is_revoked(server: &TestServer, bearer: &str, user: &TestUser, token: TokenUuid) -> bool {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/tokens?revoked=true", user.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let revoked: JsonTokens = resp.json().await.expect("Failed to parse tokens");
    revoked.0.iter().any(|revoked| revoked.uuid == token)
}

#[cfg(feature = "plus")]
async fn add_sso(server: &TestServer, admin: &TestUser, org: &TestOrg, domain: &str) {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/sso", org.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&serde_json::json!({ "domain": domain }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    assert!(
        status.is_success(),
        "{status}: {}",
        resp.text().await.unwrap_or_default()
    );
}

#[cfg(feature = "plus")]
async fn view_org(server: &TestServer, bearer: &str, org: &TestOrg) -> StatusCode {
    server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}", org.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .send()
        .await
        .expect("Request failed")
        .status()
}
