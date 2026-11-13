#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for asking to change a user's email, which waits for a link to the new email,
//! and for a server admin's change of another user's email, which applies at once.

use bencher_api_tests::{
    LogCapture, TestServer, TestServerOptions, TestUser, helpers::seed_token, session_cookie,
};
use bencher_json::{
    DateTime, Email, EmailChangeSecret, EmailChangeSecretHash, JsonUser, SessionToken,
    SessionTokenHash, TokenUuid, system::config::JsonSmtp,
};
use bencher_schema::{context::Messenger, schema};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::{Value, json};

const CONFIRM_SUBJECT: &str = "Subject: Confirm Bencher Email Change\n";
const ASKED_SUBJECT: &str = "Subject: Bencher Email Change Asked For\n";
const CHANGED_SUBJECT: &str = "Subject: Bencher Email Changed\n";
const LINK: &str = "Confirm Email: ";
const SAME_ORIGIN: (&str, &str) = ("Sec-Fetch-Site", "same-origin");

// Asking writes the user's one change, bound to the session that asked, and mails the new address
// its link, with the secret in the fragment, and the old address a notice first, which names the
// new address and links to nothing to act on. The new email waits, so nothing else changes.
// Kills an ask that records no session or another, stores the secret, puts it in a query, mails
// only one address, mails the link before the notice, or applies the email, revokes a token, or
// clears the proof of the current email.
#[tokio::test]
async fn ask_binds_the_session_and_mails_both_addresses() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Ada", "ada@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let verified = stored_email_verified(&server, &user);
    assert!(verified.is_some());
    let new_email = email("ada@lovelace.dev");

    let session = server.session(&user);
    let json = patch_ok(&server, &session, &user, &json!({ "email": new_email })).await;
    assert_eq!(json.email, user.email);
    assert_eq!(stored_email(&server, &user), user.email);
    assert_eq!(stored_email_verified(&server, &user), verified);
    assert_eq!(revoked(&server, api_token.uuid), None);

    let secret = mailed_secret(&capture, &new_email);
    let (session_id, old, new, hash) = schema::email_change::table
        .inner_join(schema::user::table)
        .filter(schema::user::uuid.eq(user.uuid))
        .select((
            schema::email_change::session_id,
            schema::email_change::email,
            schema::email_change::new_email,
            schema::email_change::secret_hash,
        ))
        .first::<(Option<i32>, String, String, String)>(&mut server.db_conn())
        .expect("No email change");
    assert_eq!(session_id, Some(session_id_of(&server, &session)));
    assert_eq!(
        (old.as_str(), new.as_str()),
        (user.email.as_ref(), new_email.as_ref())
    );
    assert_eq!(hash, EmailChangeSecretHash::from(&secret).to_string());
    assert_ne!(hash, secret.as_ref());

    let notices = emails(&capture, &user.email, ASKED_SUBJECT);
    assert_eq!(notices.len(), 1, "{notices:?}");
    let notice = notices.first().expect("No notice");
    assert!(notice.contains(new_email.as_ref()), "{notice}");
    assert_eq!(
        links(notice),
        vec!["http://localhost:3000/help"],
        "{notice}"
    );
    let lines = capture.lines();
    let notice_at = lines.iter().position(|line| line.contains(ASKED_SUBJECT));
    let link_at = lines.iter().position(|line| line.contains(CONFIRM_SUBJECT));
    assert!(notice_at < link_at, "{notice_at:?} {link_at:?}");
}

// A second ask keeps one change per user, renewed with a new secret, so the first link reads as
// expired and the second one waits.
#[tokio::test]
async fn ask_again_renews_the_change_and_ends_the_first_link() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Ada", "ada@example.com").await;
    let session = server.session(&user);
    let first_email = email("ada@first.example");
    let second_email = email("ada@second.example");

    patch_ok(&server, &session, &user, &json!({ "email": first_email })).await;
    let first = mailed_secret(&capture, &first_email);
    patch_ok(&server, &session, &user, &json!({ "email": second_email })).await;
    let second = mailed_secret(&capture, &second_email);

    assert_eq!(
        schema::email_change::table
            .count()
            .get_result::<i64>(&mut server.db_conn())
            .expect("Failed to count"),
        1
    );
    let (status, json) = read(&server, &first).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error_code"], "email_change_expired");
    let (status, json) = read(&server, &second).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["new_email"], second_email.as_ref());
}

// Each email is sent before the answer, so one that fails is a 503, after the change is written.
#[tokio::test]
async fn ask_waits_for_its_emails_and_a_failed_send_is_unavailable() {
    let server = TestServer::new_with_options(TestServerOptions {
        messenger: Some(unreachable_smtp()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server.signup("Ada", "ada@example.com").await;

    let (status, text) = patch(
        &server,
        &server.session(&user),
        &user,
        &json!({ "email": "ada@lovelace.dev" }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{text}");
    assert_eq!(
        schema::email_change::table
            .select(schema::email_change::new_email)
            .first::<String>(&mut server.db_conn())
            .expect("No email change"),
        "ada@lovelace.dev"
    );
    assert_eq!(stored_email(&server, &user), user.email);
}

// A server admin's change of another user's email is written before its notices, so a notice that
// fails to send is a 503 after the change has taken effect: the email is the new one and every
// session of the user has ended.
#[tokio::test]
async fn admin_change_waits_for_its_notices_and_a_failed_send_is_unavailable() {
    let server = TestServer::new_with_options(TestServerOptions {
        messenger: Some(unreachable_smtp()),
        ..TestServerOptions::default()
    })
    .await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let user = server.signup("Ada", "ada@example.com").await;
    let session = server.session(&user);

    let (status, text) = patch(
        &server,
        &server.session(&admin),
        &user,
        &json!({ "email": "ada@lovelace.dev" }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{text}");
    assert_eq!(stored_email(&server, &user).as_ref(), "ada@lovelace.dev");
    assert!(!session_exists(&server, &session));
}

// A new email another user already has asks for nothing, mails nothing, and applies no other field.
#[tokio::test]
async fn ask_for_a_taken_email_conflicts() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Ada", "ada@example.com").await;
    let other = server.signup("Grace", "grace@example.com").await;

    let (status, text) = patch(
        &server,
        &server.session(&user),
        &user,
        &json!({ "name": "Renamed", "email": other.email }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    assert!(text.contains("The email is already in use"), "{text}");
    assert_eq!(
        emails(&capture, &other.email, CONFIRM_SUBJECT),
        Vec::<String>::new()
    );
    assert_eq!(
        emails(&capture, &user.email, ASKED_SUBJECT),
        Vec::<String>::new()
    );
    assert_eq!(stored_name(&server, &user), user.name.as_ref());
    assert_eq!(
        schema::email_change::table
            .count()
            .get_result::<i64>(&mut server.db_conn())
            .expect("Failed to count"),
        0
    );
}

// The other fields of a PATCH with a new email apply at once.
#[tokio::test]
async fn ask_applies_the_other_fields_at_once() {
    let (server, capture) = server_with_log().await;
    let user = server.signup("Ada", "ada@example.com").await;
    let new_email = email("ada@lovelace.dev");

    let json = patch_ok(
        &server,
        &server.session(&user),
        &user,
        &json!({ "name": "Renamed", "slug": "renamed", "email": new_email }),
    )
    .await;
    assert_eq!(json.name.as_ref(), "Renamed");
    let slug: &str = json.slug.as_ref();
    assert_eq!(slug, "renamed");
    assert_eq!(json.email, user.email);
    mailed_secret(&capture, &new_email);
}

// A server admin's own new email waits for its link, as anyone's does.
#[tokio::test]
async fn admin_asks_for_their_own_email() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let new_email = email("admin@lovelace.dev");

    let json = patch_ok(
        &server,
        &server.session(&admin),
        &admin,
        &json!({ "email": new_email }),
    )
    .await;
    assert_eq!(json.email, admin.email);
    mailed_secret(&capture, &new_email);
}

// A server admin's change of another user's email applies at once and unproven, revokes that
// user's legacy tokens, ends every session of theirs and no one else's, asks for no change, and
// tells both addresses, with nothing to act on.
// Kills a direct change that keeps a session or a token, ends the admin's session, mails one
// address, or carries a link.
#[tokio::test]
async fn admin_changes_another_users_email_at_once_and_tells_both() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let user = server.signup("Ada", "ada@example.com").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let user_sessions = [server.session(&user), server.session(&user)];
    let admin_session = server.session(&admin);
    let new_email = email("ada@lovelace.dev");

    let json = patch_ok(
        &server,
        &admin_session,
        &user,
        &json!({ "email": new_email }),
    )
    .await;
    assert_eq!(json.email, new_email);
    assert_eq!(stored_email(&server, &user), new_email);
    assert_eq!(stored_email_verified(&server, &user), None);
    assert!(revoked(&server, api_token.uuid).is_some());
    for session in &user_sessions {
        assert!(!session_exists(&server, session));
    }
    assert!(session_exists(&server, &admin_session));
    assert_eq!(
        schema::email_change::table
            .count()
            .get_result::<i64>(&mut server.db_conn())
            .expect("Failed to count"),
        0
    );
    assert_eq!(
        emails(&capture, &new_email, CONFIRM_SUBJECT),
        Vec::<String>::new()
    );

    for (to, other) in [(&user.email, &new_email), (&new_email, &user.email)] {
        let notices = emails(&capture, to, CHANGED_SUBJECT);
        assert_eq!(notices.len(), 1, "{to}: {notices:?}");
        let notice = notices.first().expect("No notice");
        assert!(notice.contains(other.as_ref()), "{notice}");
        assert_eq!(
            links(notice),
            vec!["http://localhost:3000/help"],
            "{notice}"
        );
    }
}

// A server admin's change to an email another user has answers with the same words as a user's
// own, not the user's fields, and changes nothing.
#[tokio::test]
async fn admin_change_to_a_taken_email_conflicts_in_the_same_words() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let user = server.signup("Ada", "ada@example.com").await;
    let other = server.signup("Grace", "grace@example.com").await;
    let session = server.session(&user);

    let (status, text) = patch(
        &server,
        &server.session(&admin),
        &user,
        &json!({ "email": other.email }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    let json: Value = serde_json::from_str(&text).expect("Invalid body");
    let message = json["message"].as_str().expect("No message");
    assert!(message.contains("The email is already in use"), "{message}");
    for field in ["email_verified", "QueryUser", "JsonUpdateUser", "locked"] {
        assert!(!message.contains(field), "{message}");
    }
    assert_eq!(stored_email(&server, &user), user.email);
    assert!(session_exists(&server, &session));
    assert_eq!(
        emails(&capture, &user.email, CHANGED_SUBJECT),
        Vec::<String>::new()
    );
}

// A lock ends the user's pending email change, as it ends every credential, so an unlock does not
// bring back a link an attacker's session asked for.
// Kills a lock that leaves the change to confirm once the user is unlocked.
#[tokio::test]
async fn lock_and_unlock_end_the_pending_change() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let user = server.signup("Ada", "ada@example.com").await;
    let new_email = email("mallory@new.dev");
    patch_ok(
        &server,
        &server.session(&user),
        &user,
        &json!({ "email": new_email }),
    )
    .await;
    let secret = mailed_secret(&capture, &new_email);

    for locked in [true, false] {
        patch_ok(
            &server,
            &server.session(&admin),
            &user,
            &json!({ "locked": locked }),
        )
        .await;
    }

    for (status, json) in [
        read(&server, &secret).await,
        confirm(&server, &secret).await,
    ] {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert_eq!(json["error_code"], "email_change_expired");
    }
    assert_eq!(stored_email(&server, &user), user.email);
}

// A server admin's change of the user's email ends their pending email change, so changing the
// address back does not bring back its link.
// Kills an admin's change that leaves the change to confirm once the old address returns.
#[tokio::test]
async fn admin_change_and_change_back_end_the_pending_change() {
    let (server, capture) = server_with_log().await;
    let admin = server.signup("Admin", "admin@example.com").await;
    let user = server.signup("Ada", "ada@example.com").await;
    let new_email = email("mallory@new.dev");
    patch_ok(
        &server,
        &server.session(&user),
        &user,
        &json!({ "email": new_email }),
    )
    .await;
    let secret = mailed_secret(&capture, &new_email);

    for moved in ["ada@moved.dev", user.email.as_ref()] {
        patch_ok(
            &server,
            &server.session(&admin),
            &user,
            &json!({ "email": moved }),
        )
        .await;
    }

    for (status, json) in [
        read(&server, &secret).await,
        confirm(&server, &secret).await,
    ] {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert_eq!(json["error_code"], "email_change_expired");
    }
    assert_eq!(stored_email(&server, &user), user.email);
}

async fn server_with_log() -> (TestServer, LogCapture) {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    (server, capture)
}

fn email(email: &str) -> Email {
    email.parse().expect("Invalid email")
}

async fn patch_ok(
    server: &TestServer,
    session: &SessionToken,
    user: &TestUser,
    body: &Value,
) -> JsonUser {
    let (status, text) = patch(server, session, user, body).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str(&text).expect("Failed to parse user")
}

/// Patch the user from the console in `session`, signed in just now.
async fn patch(
    server: &TestServer,
    session: &SessionToken,
    user: &TestUser,
    body: &Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user.uuid)))
        .header("Cookie", session_cookie(session))
        .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
        .json(body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read response"))
}

async fn read(server: &TestServer, secret: &EmailChangeSecret) -> (StatusCode, Value) {
    post_secret(server, "/v0/auth/email/change", secret).await
}

async fn confirm(server: &TestServer, secret: &EmailChangeSecret) -> (StatusCode, Value) {
    post_secret(server, "/v0/auth/email/change/confirm", secret).await
}

async fn post_secret(
    server: &TestServer,
    path: &str,
    secret: &EmailChangeSecret,
) -> (StatusCode, Value) {
    let resp = server
        .client
        .post(server.api_url(path))
        .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
        .json(&json!({ "secret": secret.as_ref() }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.json().await.unwrap_or(Value::Null))
}

/// Every email the server logged in place of sending it to `to` with `subject`, oldest first.
fn emails(capture: &LogCapture, to: &Email, subject: &str) -> Vec<String> {
    let to = format!("<{to}>");
    capture
        .lines()
        .into_iter()
        .filter(|line| line.contains(&to) && line.contains(subject))
        .collect()
}

/// The secret of the one confirm link mailed to `new_email`, which rides in the fragment of the
/// console's confirm page.
fn mailed_secret(capture: &LogCapture, new_email: &Email) -> EmailChangeSecret {
    let mailed = emails(capture, new_email, CONFIRM_SUBJECT);
    let mail = mailed.last().expect("No confirm link was mailed");
    let (_, rest) = mail.split_once(LINK).expect("No link in the email");
    let link = rest.split_whitespace().next().expect("No link");
    let secret = link
        .strip_prefix("http://localhost:3000/auth/email#secret=")
        .expect("The secret is not in the confirm page's fragment");
    secret.parse().expect("Invalid secret")
}

/// Every URL in an email.
fn links(email: &str) -> Vec<&str> {
    email
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|word| word.starts_with("http://") || word.starts_with("https://"))
        .collect()
}

fn session_id_of(server: &TestServer, session: &SessionToken) -> i32 {
    schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(session)))
        .select(schema::session::id)
        .first(&mut server.db_conn())
        .expect("No session")
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

fn stored_email(server: &TestServer, user: &TestUser) -> Email {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::email)
        .first(&mut server.db_conn())
        .expect("Failed to get the user's email")
}

fn stored_name(server: &TestServer, user: &TestUser) -> String {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::name)
        .first(&mut server.db_conn())
        .expect("Failed to get the user's name")
}

fn stored_email_verified(server: &TestServer, user: &TestUser) -> Option<DateTime> {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::email_verified)
        .first(&mut server.db_conn())
        .expect("Failed to get when the user's email was proven")
}

fn revoked(server: &TestServer, token: TokenUuid) -> Option<DateTime> {
    schema::token::table
        .filter(schema::token::uuid.eq(token))
        .select(schema::token::revoked)
        .first(&mut server.db_conn())
        .expect("Failed to read the token")
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
