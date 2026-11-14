#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for reading and accepting an invitation by the secret its link carries, and
//! for reading an organization to claim.

#[cfg(feature = "plus")]
use bencher_api_tests::TestServerOptions;
use bencher_api_tests::{
    TestOrg, TestServer, TestUser,
    helpers::{seed_token, user_organizations},
    session_cookie,
};
use bencher_json::{
    DateTime, InviteSecret, JsonInvitation, JsonOrganization, OrganizationUuid, SessionToken,
};
#[cfg(feature = "plus")]
use bencher_json::{
    RELAY_CLIENT_IP, RELAY_SECRET as RELAY_SECRET_HEADER, system::config::JsonRateLimiting,
};
#[cfg(feature = "plus")]
use bencher_schema::context::RateLimiting;
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::{Value, json};

const DAY: i64 = 24 * 60 * 60;
const SAME_ORIGIN: (&str, &str) = ("Sec-Fetch-Site", "same-origin");
const CROSS_SITE: (&str, &str) = ("Sec-Fetch-Site", "cross-site");

// The read names the invitation's email, organization, role, and expiration to anyone with the
// secret, and to a browser session also whether it is for the session's user.
// Kills a read that names the wrong invitation, tells a session nothing, or says yes to another
// account.
#[tokio::test]
async fn read_names_the_invitation_and_whom_it_is_for() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");

    for (session, for_you) in [
        (None, None),
        (Some(server.session(&grace)), Some(true)),
        (Some(server.session(&leader)), Some(false)),
    ] {
        let (status, json) = read(&server, &secret, session.as_ref(), SAME_ORIGIN).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let invitation: JsonInvitation =
            serde_json::from_value(json.clone()).expect("Invalid invitation");
        assert_eq!(invitation.email.as_ref(), "grace@example.com");
        assert_eq!(invitation.organization.name, org.name);
        assert_eq!(invitation.organization.slug, org.slug);
        assert_eq!(json["role"], "leader");
        assert!(invitation.expiration.timestamp() > DateTime::now().timestamp());
        assert_eq!(invitation.for_you, for_you, "{json}");
        assert!(!json.to_string().contains(secret.as_ref()));
    }
}

// An invitation no row holds, one that ended, and one that expired are one refusal, which tells
// none of them apart.
#[tokio::test]
async fn read_refuses_unknown_ended_and_expired_alike() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let unknown = InviteSecret::generate().expect("Failed to make a secret");
    let ended = server.invite(&leader, &org, "grace@example.com");
    assert_eq!(
        accept(&server, &ended, Some(&server.session(&grace)))
            .await
            .0,
        StatusCode::OK
    );
    let expired = server.invite_at(
        &leader,
        &org,
        "alan@example.com",
        DateTime::try_from(DateTime::now().timestamp() - 7 * DAY).expect("Invalid time"),
    );

    let refusals: Vec<(StatusCode, Value)> = [
        read(&server, &unknown, None, SAME_ORIGIN).await,
        read(&server, &ended, None, SAME_ORIGIN).await,
        read(&server, &expired, None, SAME_ORIGIN).await,
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
    assert_eq!(first.1["error_code"], "invalid_invite");
    assert!(
        refusals.iter().all(|refusal| *refusal == first),
        "{refusals:?}"
    );
}

// Both endpoints serve the console alone.
#[tokio::test]
async fn read_and_accept_need_the_console_origin() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");

    let (status, json) = read(&server, &secret, None, CROSS_SITE).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error_code"], "cross_origin");
    let (status, json) = post(
        &server,
        "/v0/auth/invite/accept",
        &secret,
        Some(&server.session(&grace)),
        CROSS_SITE,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error_code"], "cross_origin");
    assert!(!user_organizations(&server, grace.uuid).contains(&org.uuid));
}

// The read is charged to the reader's address, as starting a sign-in is, so the secrets cannot be
// guessed faster than sign-ins can start.
#[cfg(feature = "plus")]
#[tokio::test]
async fn read_is_charged_to_the_readers_address() {
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
    let unknown = InviteSecret::generate().expect("Failed to make a secret");
    let relayed = |ip: &'static str| {
        server
            .client
            .post(server.api_url("/v0/auth/invite"))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .header(RELAY_SECRET_HEADER, RELAY_SECRET)
            .header(RELAY_CLIENT_IP, ip)
            .json(&json!({"secret": unknown.as_ref()}))
    };
    for _ in 0..2 {
        let resp = relayed("203.0.113.7").send().await.expect("Request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    let resp = relayed("203.0.113.7").send().await.expect("Request failed");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let json: Value = resp.json().await.expect("Invalid body");
    assert_eq!(json["error_code"], "too_many_attempts");
    let resp = relayed("203.0.113.8").send().await.expect("Request failed");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// Accepting joins the session's user with the invitation's role and ends the invitation, so a
// second press finds it not valid.
#[tokio::test]
async fn accept_joins_with_the_rows_role_and_ends_it() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");
    let session = server.session(&grace);

    let (status, json) = accept(&server, &secret, Some(&session)).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let organization: JsonOrganization =
        serde_json::from_value(json).expect("Invalid organization");
    assert_eq!(organization.uuid, org.uuid);
    assert_eq!(role(&server, &grace, &org).as_deref(), Some("leader"));
    assert_eq!(invites(&server), 0);

    let (status, json) = accept(&server, &secret, Some(&session)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error_code"], "invalid_invite");
}

// An invitation for another account's email refuses it and stays for its own address; a key, a
// deprecated API token, an old session token, and no credential at all accept nothing.
// Kills an accept by an account the invitation is not for, or by any credential but a browser
// session.
#[tokio::test]
async fn accept_needs_the_invited_accounts_browser_session() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let carol = server.signup("Carol", "carol@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");

    let (status, json) = accept(&server, &secret, Some(&server.session(&carol))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error_code"], "invite_for_another_email");
    assert!(!user_organizations(&server, carol.uuid).contains(&org.uuid));
    assert_eq!(invites(&server), 1);

    let key = server.create_user_key(&grace, "Key").await;
    let token = seed_token(&server, &grace, "Token").token;
    for bearer in [key.as_ref(), token.as_ref(), grace.token.as_str()] {
        let resp = server
            .client
            .post(server.api_url("/v0/auth/invite/accept"))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .json(&json!({"secret": secret.as_ref()}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(
        accept(&server, &secret, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(!user_organizations(&server, grace.uuid).contains(&org.uuid));
    assert_eq!(invites(&server), 1);
}

// Of many presses at once, one joins and the rest find the invitation ended.
#[tokio::test(flavor = "multi_thread")]
async fn parallel_accepts_join_once() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let secret = server.invite(&leader, &org, "grace@example.com");
    let sessions: Vec<SessionToken> = std::iter::repeat_with(|| server.session(&grace))
        .take(8)
        .collect();

    let mut requests = tokio::task::JoinSet::new();
    for session in &sessions {
        let request = server
            .client
            .post(server.api_url("/v0/auth/invite/accept"))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .header("Cookie", session_cookie(session))
            .json(&json!({"secret": secret.as_ref()}));
        requests.spawn(async move {
            let resp = request.send().await.expect("Request failed");
            let status = resp.status();
            let json: Value = resp.json().await.unwrap_or(Value::Null);
            (status, json)
        });
    }
    let statuses = requests.join_all().await;
    assert_eq!(
        statuses
            .iter()
            .filter(|(status, _)| *status == StatusCode::OK)
            .count(),
        1,
        "{statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|(status, json)| *status == StatusCode::OK
                || json["error_code"] == "invalid_invite"),
        "{statuses:?}"
    );
    assert_eq!(organization_roles(&server, &org), 2);
}

// A reader who already belongs to the organization ends the invitation and lands in it, as if the
// press joined them; an expired invitation and a revoked one accept nothing.
#[tokio::test]
async fn already_a_member_ends_it_and_expired_or_revoked_refuse() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited Org").await;
    let grace = server.signup("Grace", "grace@example.com").await;

    let own = server.invite(&leader, &org, "leader@example.com");
    let (status, json) = accept(&server, &own, Some(&server.session(&leader))).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["uuid"], org.uuid.to_string());
    assert_eq!(invites(&server), 0);
    assert_eq!(organization_roles(&server, &org), 1);

    let expired = server.invite_at(
        &leader,
        &org,
        "grace@example.com",
        DateTime::try_from(DateTime::now().timestamp() - 7 * DAY).expect("Invalid time"),
    );
    let (status, json) = accept(&server, &expired, Some(&server.session(&grace))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error_code"], "invalid_invite");

    let revoked = server.invite(&leader, &org, "grace@example.com");
    diesel::delete(schema::invite::table)
        .execute(&mut server.db_conn())
        .expect("Failed to revoke");
    let (status, json) = accept(&server, &revoked, Some(&server.session(&grace))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error_code"], "invalid_invite");
    assert!(!user_organizations(&server, grace.uuid).contains(&org.uuid));
}

// The legacy sign up and sign in refuse an invitation, which only the new sign-in carries.
#[tokio::test]
async fn legacy_routes_refuse_an_invite() {
    let server = TestServer::new().await;
    server.signup("Leader", "leader@example.com").await;
    for (path, body) in [
        (
            "/v0/auth/signup",
            json!({
                "name": "Grace",
                "email": "grace@example.com",
                "invite": bencher_json::Jwt::test_token(),
                "i_agree": true,
            }),
        ),
        (
            "/v0/auth/login",
            json!({"email": "leader@example.com", "invite": bencher_json::Jwt::test_token()}),
        ),
    ] {
        let resp = server
            .client
            .post(server.api_url(path))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{path}");
        let json: Value = resp.json().await.expect("Invalid body");
        assert_eq!(json["error_code"], "invalid_invite", "{path}");
    }
    let users: i64 = schema::user::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count");
    assert_eq!(users, 1);
}

// The console names an organization before its Claim press only while no one has claimed it: a
// claimed organization is a conflict, and one that is gone or never was is unknown.
// Kills a read that names any organization by its UUID to anyone.
#[tokio::test]
async fn claim_read_names_only_an_unclaimed_organization() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let claimed = server.create_org(&leader, "Claimed Org").await;
    let unclaimed = OrganizationUuid::new();
    diesel::insert_into(schema::organization::table)
        .values((
            schema::organization::uuid.eq(unclaimed.to_string()),
            schema::organization::name.eq("Unclaimed Org"),
            schema::organization::slug.eq("unclaimed-org"),
            schema::organization::created.eq(0i64),
            schema::organization::modified.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to add an organization");
    let read = |organization: OrganizationUuid| {
        server
            .client
            .get(server.api_url(&format!("/v0/auth/claim/{organization}")))
            .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
            .send()
    };

    let resp = read(unclaimed).await.expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = resp.json().await.expect("Invalid body");
    assert_eq!(
        json,
        json!({"name": "Unclaimed Org", "slug": "unclaimed-org"})
    );

    let resp = read(claimed.uuid).await.expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let json: Value = resp.json().await.expect("Invalid body");
    assert!(!json.to_string().contains("Claimed Org"), "{json}");

    diesel::update(
        schema::organization::table.filter(schema::organization::uuid.eq(unclaimed.to_string())),
    )
    .set(schema::organization::deleted.eq(Some(0i64)))
    .execute(&mut server.db_conn())
    .expect("Failed to delete the organization");
    for organization in [unclaimed, OrganizationUuid::new()] {
        let resp = read(organization).await.expect("Request failed");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let json: Value = resp.json().await.expect("Invalid body");
        assert_eq!(json["error_code"], "unknown_organization");
    }
}

async fn read(
    server: &TestServer,
    secret: &InviteSecret,
    session: Option<&SessionToken>,
    origin: (&str, &str),
) -> (StatusCode, Value) {
    post(server, "/v0/auth/invite", secret, session, origin).await
}

async fn accept(
    server: &TestServer,
    secret: &InviteSecret,
    session: Option<&SessionToken>,
) -> (StatusCode, Value) {
    post(
        server,
        "/v0/auth/invite/accept",
        secret,
        session,
        SAME_ORIGIN,
    )
    .await
}

async fn post(
    server: &TestServer,
    path: &str,
    secret: &InviteSecret,
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

fn invites(server: &TestServer) -> i64 {
    schema::invite::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn organization_roles(server: &TestServer, org: &TestOrg) -> i64 {
    schema::organization_role::table
        .inner_join(schema::organization::table)
        .filter(schema::organization::uuid.eq(org.uuid.to_string()))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn role(server: &TestServer, user: &TestUser, org: &TestOrg) -> Option<String> {
    schema::organization_role::table
        .inner_join(schema::organization::table)
        .inner_join(schema::user::table)
        .filter(schema::organization::uuid.eq(org.uuid.to_string()))
        .filter(schema::user::uuid.eq(user.uuid.to_string()))
        .select(schema::organization_role::role)
        .first(&mut server.db_conn())
        .ok()
}
