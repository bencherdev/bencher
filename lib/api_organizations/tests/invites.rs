#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for invitations: inviting by a row bound to the email, the pending list, Resend,
//! and Revoke, each with the credentials it takes.

use bencher_api_tests::{
    LogCapture, TestOrg, TestServer, TestServerOptions, TestUser, helpers::seed_token,
    session_cookie,
};
use bencher_json::{
    DateTime, InviteSecret, InviteSecretHash, JsonInvite, JsonInvites, JsonMember, SessionToken,
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{Method, StatusCode};
use serde_json::{Value, json};

const MINUTE: i64 = 60;
const DAY: i64 = 24 * 60 * MINUTE;
const CONSOLE: &str = "http://localhost:3000";

// An invitation is a row bound to its email, its organization, its role, and its inviter, which
// lives 7 days and keeps only the hash of the secret its email's link carries in the fragment.
// Kills an invitation that stores its secret, links anywhere but the door's fragment, or lives
// another length.
#[tokio::test]
async fn invite_makes_a_row_and_emails_its_secret() {
    let (server, log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;

    let resp = invite(
        &server,
        &Credential::Fresh(&leader),
        &org,
        "grace@example.com",
    )
    .await;
    assert_eq!(resp.0, StatusCode::ACCEPTED, "{}", resp.1);
    assert_eq!(resp.1["email"], "grace@example.com");

    let secret = emailed_secret(&log);
    let rows = invite_rows(&server);
    let [(email, role, inviter, secret_hash, created, expiration)] = rows.as_slice() else {
        panic!("{rows:?}");
    };
    assert_eq!(email, "grace@example.com");
    assert_eq!(role, "leader");
    assert_eq!(*inviter, user_id(&server, &leader));
    assert_eq!(*secret_hash, InviteSecretHash::from(&secret).to_string());
    assert_eq!(expiration - created, 7 * DAY);
    let email = log.email("Join Invites");
    assert!(!email.contains("Invite Code"), "{email}");
    assert_eq!(email.matches(secret.as_ref()).count(), 1, "{email}");
}

// A user key and a deprecated API token get the session-only refusal, a session that signed in
// more than 10 minutes ago gets the fresh sign-in refusal, and none of them invites anyone.
// Kills an invite any credential may send, or one a stolen stale cookie may.
#[tokio::test]
async fn inviting_needs_a_fresh_browser_session() {
    let (server, log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let key = server.create_user_key(&leader, "Key").await;
    let token = seed_token(&server, &leader, "Token").token;
    let stale = stale_session(&server, &leader);

    for credential in [
        Credential::Bearer(key.as_ref()),
        Credential::Bearer(token.as_ref()),
    ] {
        let (status, json) = invite(&server, &credential, &org, "grace@example.com").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{json}");
        assert!(
            json["message"]
                .as_str()
                .is_some_and(|message| message.contains("Only a user session")),
            "{json}"
        );
    }
    let (status, json) = invite(
        &server,
        &Credential::Session(&stale),
        &org,
        "grace@example.com",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{json}");
    assert_eq!(json["error_code"], "fresh_sign_in");
    assert_eq!(invite_rows(&server), Vec::new());
    assert!(
        log.lines()
            .iter()
            .all(|line| !line.contains("Invitation to join")),
        "no email went out"
    );
}

// An email that already belongs to a member is a conflict, and no invitation is made.
#[tokio::test]
async fn invite_to_a_members_email_conflicts() {
    let (server, _log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let (status, json) = invite(
        &server,
        &Credential::Fresh(&leader),
        &org,
        "LEADER@example.com",
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{json}");
    assert_eq!(invite_rows(&server), Vec::new());
}

// A second invitation to an address renews its row: one row with the same UUID, a new secret, a new
// 7 days, and the second inviter, so the earlier link reads as not valid.
#[tokio::test]
async fn second_invite_renews_the_row_and_ends_the_earlier_link() {
    let (server, log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let second = server.signup("Second", "second@example.com").await;
    let earlier = server.invite_at(
        &leader,
        &org,
        "grace@example.com",
        DateTime::try_from(DateTime::now().timestamp() - DAY).expect("Invalid time"),
    );
    join(&server, &leader, &org, &second).await;
    let before = list(&server, &leader, &org).await;

    let (status, json) = invite(
        &server,
        &Credential::Fresh(&second),
        &org,
        "grace@example.com",
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{json}");
    let after = list(&server, &leader, &org).await;
    let ([before], [after]) = (before.0.as_slice(), after.0.as_slice()) else {
        panic!("{before:?} {after:?}");
    };
    assert_eq!(after.uuid, before.uuid);
    assert_eq!(after.inviter.uuid, second.uuid);
    assert!(after.expiration.timestamp() > before.expiration.timestamp());
    assert_eq!(read(&server, &earlier).await, StatusCode::BAD_REQUEST);
    assert_eq!(read(&server, &emailed_secret(&log)).await, StatusCode::OK);
}

// The list holds every invitation not yet accepted or revoked, the expired included, with its
// email, role, inviter, and dates, and neither a secret nor a hash; and only a reader who may
// invite reads it.
#[tokio::test]
async fn list_holds_pending_invites_without_secrets() {
    let (server, _log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let outsider = server.signup("Outsider", "outsider@example.com").await;
    let expired_at =
        DateTime::try_from(DateTime::now().timestamp() - 8 * DAY).expect("Invalid time");
    let expired = server.invite_at(&leader, &org, "expired@example.com", expired_at);
    let live = server.invite(&leader, &org, "grace@example.com");

    let (status, json) = send(
        &server,
        Method::GET,
        &format!("/v0/organizations/{}/invites", org.slug),
        &Credential::Bearer(&leader.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let text = json.to_string();
    for secret in [&expired, &live] {
        assert!(!text.contains(secret.as_ref()), "{text}");
        assert!(
            !text.contains(InviteSecretHash::from(secret).as_ref()),
            "{text}"
        );
    }
    let invites: JsonInvites = serde_json::from_value(json).expect("Invalid invites");
    let emails: Vec<String> = invites.0.iter().map(|i| i.email.to_string()).collect();
    assert_eq!(emails, ["grace@example.com", "expired@example.com"]);
    let first = invites.0.first().expect("No invitation");
    assert_eq!(first.inviter.slug, leader.slug);
    assert_eq!(first.inviter.name, leader.name);

    let (status, _json) = send(
        &server,
        Method::GET,
        &format!("/v0/organizations/{}/invites", org.slug),
        &Credential::Bearer(&outsider.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Resend gives the invitation, expired or not, a new secret and 7 more days in a new email, and
// takes the same credentials as inviting.
// Kills a Resend any credential may send, one a stale cookie may, and one that keeps the old link.
#[tokio::test]
async fn resend_renews_with_the_inviting_credentials() {
    let (server, log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let expired_at =
        DateTime::try_from(DateTime::now().timestamp() - 8 * DAY).expect("Invalid time");
    let earlier = server.invite_at(&leader, &org, "grace@example.com", expired_at);
    let [pending] = list(&server, &leader, &org)
        .await
        .0
        .try_into()
        .expect("One invitation");
    let path = format!(
        "/v0/organizations/{}/invites/{}/resend",
        org.slug, pending.uuid
    );
    let key = server.create_user_key(&leader, "Key").await;
    let stale = stale_session(&server, &leader);

    let (status, _json) = send(
        &server,
        Method::POST,
        &path,
        &Credential::Bearer(key.as_ref()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, json) = send(
        &server,
        Method::POST,
        &path,
        &Credential::Session(&stale),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error_code"], "fresh_sign_in");
    assert_eq!(
        list(&server, &leader, &org)
            .await
            .0
            .first()
            .map(|i| i.expiration),
        Some(pending.expiration)
    );

    let (status, json) = send(
        &server,
        Method::POST,
        &path,
        &Credential::Fresh(&leader),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{json}");
    let resent: JsonInvite = serde_json::from_value(json).expect("Invalid invitation");
    assert_eq!(resent.uuid, pending.uuid);
    assert_eq!(
        resent.expiration.timestamp() - resent.created.timestamp(),
        7 * DAY
    );
    assert!(resent.expiration.timestamp() > DateTime::now().timestamp());
    assert_eq!(read(&server, &earlier).await, StatusCode::BAD_REQUEST);
    assert_eq!(read(&server, &emailed_secret(&log)).await, StatusCode::OK);
}

// Revoke ends the invitation for a reader who may remove members, with any credential and no
// fresh sign-in, so its link reads as not valid; a second Revoke finds nothing.
#[tokio::test]
async fn revoke_ends_the_invitation() {
    let (server, _log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let outsider = server.signup("Outsider", "outsider@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");
    let [pending] = list(&server, &leader, &org)
        .await
        .0
        .try_into()
        .expect("One invitation");
    let path = format!("/v0/organizations/{}/invites/{}", org.slug, pending.uuid);

    let (status, _json) = send(
        &server,
        Method::DELETE,
        &path,
        &Credential::Bearer(&outsider.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(read(&server, &secret).await, StatusCode::OK);

    let key = server.create_user_key(&leader, "Key").await;
    let (status, _json) = send(
        &server,
        Method::DELETE,
        &path,
        &Credential::Bearer(key.as_ref()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(read(&server, &secret).await, StatusCode::BAD_REQUEST);
    assert_eq!(invite_rows(&server), Vec::new());
    let (status, _json) = send(
        &server,
        Method::DELETE,
        &path,
        &Credential::Bearer(key.as_ref()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Resend needs `create_role` in the invitation's own organization: an outsider with a fresh session
// renews nothing, through that organization or through one they lead, and no email goes out.
// Kills a Resend that checks no permission, and one that finds an invitation by its UUID through
// any organization, whose email would name the outsider's organization.
#[tokio::test]
async fn resend_by_an_outsider_renews_nothing() {
    let (server, log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let outsider = server.signup("Outsider", "outsider@example.com").await;
    let own = server.create_org(&outsider, "Outsider Org").await;
    let secret = server.invite(&leader, &org, "grace@example.com");
    let [pending] = list(&server, &leader, &org)
        .await
        .0
        .try_into()
        .expect("One invitation");
    let before = invite_rows(&server);

    for slug in [&org.slug, &own.slug] {
        let (status, json) = send(
            &server,
            Method::POST,
            &format!("/v0/organizations/{slug}/invites/{}/resend", pending.uuid),
            &Credential::Fresh(&outsider),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{slug}: {json}");
    }
    assert_eq!(invite_rows(&server), before);
    assert_eq!(read(&server, &secret).await, StatusCode::OK);
    assert!(
        log.lines()
            .iter()
            .all(|line| !line.contains("Invitation to join")),
        "no email went out"
    );
}

// Revoke finds an invitation only in the organization its path names: an outsider who leads
// another organization ends nothing through it by the invitation's UUID.
// Kills an invitation found by its UUID through any organization.
#[tokio::test]
async fn revoke_through_another_organization_ends_nothing() {
    let (server, _log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let outsider = server.signup("Outsider", "outsider@example.com").await;
    let own = server.create_org(&outsider, "Outsider Org").await;
    let secret = server.invite(&leader, &org, "grace@example.com");
    let [pending] = list(&server, &leader, &org)
        .await
        .0
        .try_into()
        .expect("One invitation");
    let before = invite_rows(&server);

    let (status, json) = send(
        &server,
        Method::DELETE,
        &format!("/v0/organizations/{}/invites/{}", own.slug, pending.uuid),
        &Credential::Fresh(&outsider),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
    assert_eq!(invite_rows(&server), before);
    assert_eq!(read(&server, &secret).await, StatusCode::OK);
}

// Granting a role takes a browser session signed in within 10 minutes, as inviting does; updating
// nothing still needs a reader who may see the members.
// Kills a role grant any credential or a stale cookie may make, and a PATCH of nothing that tells
// anyone signed in a member's email.
#[tokio::test]
async fn role_grant_needs_a_fresh_session_and_nothing_needs_view_role() {
    let (server, _log) = logged_server().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invites").await;
    let member = server.signup("Member", "member@example.com").await;
    join(&server, &leader, &org, &member).await;
    let outsider = server.signup("Outsider", "outsider@example.com").await;
    let path = format!("/v0/organizations/{}/members/{}", org.slug, member.uuid);
    let grant = json!({"role": "leader"});
    let key = server.create_user_key(&leader, "Key").await;
    let stale = stale_session(&server, &leader);

    let (status, _json) = send(
        &server,
        Method::PATCH,
        &path,
        &Credential::Bearer(key.as_ref()),
        Some(&grant),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, json) = send(
        &server,
        Method::PATCH,
        &path,
        &Credential::Session(&stale),
        Some(&grant),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error_code"], "fresh_sign_in");
    let (status, json) = send(
        &server,
        Method::PATCH,
        &path,
        &Credential::Fresh(&leader),
        Some(&grant),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");

    let (status, json) = send(
        &server,
        Method::PATCH,
        &path,
        &Credential::Bearer(&outsider.token),
        Some(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!json.to_string().contains("member@example.com"), "{json}");
    let (status, json) = send(
        &server,
        Method::PATCH,
        &path,
        &Credential::Bearer(key.as_ref()),
        Some(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let member: JsonMember = serde_json::from_value(json).expect("Invalid member");
    assert_eq!(member.email.as_ref(), "member@example.com");
}

/// How a request presents its reader.
enum Credential<'a> {
    /// `Authorization: Bearer`, from anywhere.
    Bearer(&'a str),
    /// A session's cookie, from the console.
    Session(&'a SessionToken),
    /// A session the user signs in to now, from the console.
    Fresh(&'a TestUser),
}

async fn send(
    server: &TestServer,
    method: Method,
    path: &str,
    credential: &Credential<'_>,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let request = server.client.request(method, server.api_url(path));
    let request = match credential {
        Credential::Bearer(bearer) => request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        ),
        Credential::Session(token) => request
            .header("Cookie", session_cookie(token))
            .header("Sec-Fetch-Site", "same-origin"),
        Credential::Fresh(user) => request.headers(server.fresh_session(user)),
    };
    let request = if let Some(body) = body {
        request.json(body)
    } else {
        request
    };
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("No body");
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

async fn invite(
    server: &TestServer,
    credential: &Credential<'_>,
    org: &TestOrg,
    email: &str,
) -> (StatusCode, Value) {
    send(
        server,
        Method::POST,
        &format!("/v0/organizations/{}/members", org.slug),
        credential,
        Some(&json!({"email": email, "role": "leader"})),
    )
    .await
}

async fn list(server: &TestServer, leader: &TestUser, org: &TestOrg) -> JsonInvites {
    let (status, json) = send(
        server,
        Method::GET,
        &format!("/v0/organizations/{}/invites", org.slug),
        &Credential::Bearer(&leader.token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    serde_json::from_value(json).expect("Invalid invites")
}

/// What the console's read of an invitation answers.
async fn read(server: &TestServer, secret: &InviteSecret) -> StatusCode {
    server
        .client
        .post(server.api_url("/v0/auth/invite"))
        .header("Sec-Fetch-Site", "same-origin")
        .json(&json!({"secret": secret.as_ref()}))
        .send()
        .await
        .expect("Request failed")
        .status()
}

/// Make `member` a member of the organization, as an invitation accepted with a press does.
async fn join(server: &TestServer, leader: &TestUser, org: &TestOrg, member: &TestUser) {
    let secret = server.invite(leader, org, member.email.as_ref());
    let resp = server
        .client
        .post(server.api_url("/v0/auth/invite/accept"))
        .headers(server.fresh_session(member))
        .json(&json!({"secret": secret.as_ref()}))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

/// A session of the user's that signed in 11 minutes ago, past the window.
fn stale_session(server: &TestServer, user: &TestUser) -> SessionToken {
    server.session_at(
        user,
        DateTime::try_from(DateTime::now().timestamp() - 11 * MINUTE).expect("Invalid time"),
    )
}

/// The secret of the last invitation the server logged in place of emailing it.
fn emailed_secret(log: &LogCapture) -> InviteSecret {
    let link = log.email_link(&format!("Join {}", "Invites"));
    assert_eq!(link.path(), "/auth/login");
    assert_eq!(link.query(), None);
    assert!(link.as_str().starts_with(CONSOLE), "{link}");
    link.fragment()
        .and_then(|fragment| fragment.strip_prefix("invite="))
        .expect("No invitation in the link")
        .parse()
        .expect("Invalid secret")
}

/// Each invitation's email, role, inviter, secret hash, creation, and expiration.
fn invite_rows(server: &TestServer) -> Vec<(String, String, i32, String, i64, i64)> {
    schema::invite::table
        .order(schema::invite::id)
        .select((
            schema::invite::email,
            schema::invite::role,
            schema::invite::inviter_id,
            schema::invite::secret_hash,
            schema::invite::created,
            schema::invite::expiration,
        ))
        .load(&mut server.db_conn())
        .expect("Failed to read the invitations")
}

fn user_id(server: &TestServer, user: &TestUser) -> i32 {
    schema::user::table
        .filter(schema::user::uuid.eq(user.uuid.to_string()))
        .select(schema::user::id)
        .first(&mut server.db_conn())
        .expect("No user")
}

async fn logged_server() -> (TestServer, LogCapture) {
    let log = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        log: Some(log.logger()),
        ..TestServerOptions::default()
    })
    .await;
    (server, log)
}
