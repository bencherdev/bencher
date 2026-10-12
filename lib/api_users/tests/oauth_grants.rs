#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for OAuth grant management (`/v0/users/{user}/oauth/grants`).

use std::sync::Arc;

use bencher_api_tests::{
    LogCapture, TestServer, TestServerOptions, TestUser,
    helpers::{TestOAuthGrant, oauth_resource, oauth_server, seed_oauth_grant, seed_token},
};
use bencher_json::{
    Clock, DateTime, JsonOAuthGrant, JsonOAuthGrants, JsonUserKeyCreated, OAuthCode, OAuthScope,
    UserKey, UserKeyUuid,
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{Method, StatusCode};
use serde_json::{Value, json};

/// The time every test server here is frozen at.
const NOW: i64 = 1_800_000_000;

// Only grants that can still be used are listed: not revoked, not expired, and not bound to a dead key
#[tokio::test]
async fn oauth_grants_list_active() {
    let server = server().await;
    let user = server
        .signup("Grants User", "grants-user@example.com")
        .await;
    let session = grant(&server, &user, None);
    let (live_key, _) = create_user_key(&server, &user, "Live Key").await;
    let keyed = grant(&server, &user, Some(live_key));
    let (dead_key, _) = create_user_key(&server, &user, "Dead Key").await;
    grant(&server, &user, Some(dead_key));
    let (status, ..) = request(
        &server,
        Method::DELETE,
        &format!("/v0/users/{}/keys/{dead_key}", user.slug),
        &user.token,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (lapsed_key, _) = create_user_key(&server, &user, "Lapsed Key").await;
    let lapsed_keyed = grant(&server, &user, Some(lapsed_key));
    let revoked = grant(&server, &user, None);
    let expired = grant(&server, &user, None);
    {
        let mut conn = server.db_conn();
        diesel::update(schema::user_key::table.filter(schema::user_key::uuid.eq(lapsed_key)))
            .set(schema::user_key::expiration.eq(now()))
            .execute(&mut conn)
            .expect("Failed to expire the user key");
        diesel::update(
            schema::oauth_grant::table.filter(schema::oauth_grant::uuid.eq(revoked.uuid)),
        )
        .set(schema::oauth_grant::revoked.eq(now()))
        .execute(&mut conn)
        .expect("Failed to revoke the grant");
        diesel::update(
            schema::oauth_grant::table.filter(schema::oauth_grant::uuid.eq(expired.uuid)),
        )
        .set(schema::oauth_grant::expiration.eq(now()))
        .execute(&mut conn)
        .expect("Failed to expire the grant");
    }

    let (status, total_count, body) = request(
        &server,
        Method::GET,
        &format!("/v0/users/{}/oauth/grants", user.slug),
        &user.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total_count.as_deref(), Some("2"));
    let grants: Vec<JsonOAuthGrant> = serde_json::from_value::<JsonOAuthGrants>(body)
        .expect("Invalid grants")
        .into();
    let uuids: Vec<_> = grants.iter().map(|grant| grant.uuid).collect();
    assert_eq!(uuids, vec![session.uuid, keyed.uuid]);
    let [session_json, keyed_json] = grants.as_slice() else {
        panic!("Expected two grants: {grants:?}");
    };
    assert_eq!(session_json.client.host, "client.example");
    assert_eq!(session_json.scopes, vec![OAuthScope::ProjectsRead]);
    assert!(session_json.user_key.is_none());
    let user_key = keyed_json.user_key.as_ref().expect("Missing user key");
    assert_eq!(user_key.uuid, live_key);
    assert_eq!(user_key.name.as_ref(), "Live Key");

    let (status, total_count, page) = request(
        &server,
        Method::GET,
        &format!("/v0/users/{}/oauth/grants?per_page=1&page=2", user.slug),
        &user.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total_count.as_deref(), Some("2"));
    assert_eq!(
        page,
        json!([serde_json::to_value(keyed_json).expect("Invalid grant")])
    );

    for (grant, expected) in [
        (&keyed, StatusCode::OK),
        (&lapsed_keyed, StatusCode::NOT_FOUND),
        (&revoked, StatusCode::NOT_FOUND),
        (&expired, StatusCode::NOT_FOUND),
    ] {
        let (status, ..) =
            request(&server, Method::GET, &grant_path(&user, grant), &user.token).await;
        assert_eq!(status, expected);
    }
}

// Disconnecting an app revokes its grant, and its access token stops working at once
#[tokio::test]
async fn oauth_grant_delete_revokes() {
    let server = server().await;
    let user = server
        .signup("Grants Delete", "grants-delete@example.com")
        .await;
    let grant = grant(&server, &user, None);
    assert_eq!(mcp_status(&server, &grant).await, StatusCode::OK);

    let (status, ..) = request(
        &server,
        Method::DELETE,
        &grant_path(&user, &grant),
        &user.token,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(mcp_status(&server, &grant).await, StatusCode::UNAUTHORIZED);
    for method in [Method::GET, Method::DELETE] {
        let (status, ..) = request(&server, method, &grant_path(&user, &grant), &user.token).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

// Another user cannot disconnect a user's apps
#[tokio::test]
async fn oauth_grant_delete_other_user_forbidden() {
    let server = server().await;
    let user = server
        .signup("Grants Owner", "grants-owner@example.com")
        .await;
    let other = server
        .signup("Grants Other", "grants-other@example.com")
        .await;
    let grant = grant(&server, &user, None);

    let (status, ..) = request(
        &server,
        Method::DELETE,
        &grant_path(&user, &grant),
        &other.token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(mcp_status(&server, &grant).await, StatusCode::OK);
}

// Kills grants management open to anything but a session: neither a user API key nor a legacy
// API token can see or sever OAuth grants
#[tokio::test]
async fn oauth_grants_need_a_session() {
    let server = server().await;
    let user = server
        .signup("Grants Keyed", "grants-keyed@example.com")
        .await;
    let (_, key) = create_user_key(&server, &user, "Grants Key").await;
    let legacy = seed_token(&server, &user, "Grants Token").token;
    let grant = grant(&server, &user, None);
    let list = format!("/v0/users/{}/oauth/grants", user.slug);

    for bearer in [key.as_ref(), legacy.as_ref()] {
        for (method, path) in [
            (Method::GET, list.clone()),
            (Method::GET, grant_path(&user, &grant)),
            (Method::DELETE, grant_path(&user, &grant)),
        ] {
            let (status, ..) = request(&server, method, &path, bearer).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
        }
    }
    assert_eq!(mcp_status(&server, &grant).await, StatusCode::OK);

    let (status, ..) = request(&server, Method::GET, &list, &user.token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, ..) = request(
        &server,
        Method::DELETE,
        &grant_path(&user, &grant),
        &user.token,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// Without OAuth configured there are no grants to manage, so the endpoints are not there
#[tokio::test]
async fn oauth_grants_not_found_without_oauth() {
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(now))),
        ..TestServerOptions::default()
    })
    .await;
    let user = server
        .signup("Grants Dormant", "grants-dormant@example.com")
        .await;
    let grant = grant(&server, &user, None);

    for (method, path) in [
        (Method::GET, format!("/v0/users/{}/oauth/grants", user.slug)),
        (Method::GET, grant_path(&user, &grant)),
        (Method::DELETE, grant_path(&user, &grant)),
    ] {
        let (status, ..) = request(&server, method, &path, &user.token).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

// Kills a REST endpoint that accepts an OAuth token, or that parses a refresh token or code as a
// JSON Web Token, whose error quotes it back and into the request log
#[tokio::test]
async fn oauth_tokens_at_rest_are_unauthorized_and_never_quoted() {
    let log = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(now))),
        oauth: Some(oauth_server()),
        log: Some(log.logger()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server
        .signup("Grants Rest", "grants-rest@example.com")
        .await;
    let grant = grant(&server, &user, None);
    assert_eq!(mcp_status(&server, &grant).await, StatusCode::OK);
    let code = OAuthCode::generate();

    let tokens: [&str; 3] = [
        grant.access_token.as_ref(),
        grant.refresh_token.as_ref(),
        code.as_ref(),
    ];
    for token in tokens {
        let (status, _, body) = request(
            &server,
            Method::GET,
            &format!("/v0/users/{}", user.slug),
            token,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_ne!(body, Value::Null);
        assert!(!body.to_string().contains(token));
    }
    let lines = log.lines();
    assert!(
        tokens
            .iter()
            .all(|token| lines.iter().all(|line| !line.contains(token)))
    );
}

async fn server() -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(now))),
        oauth: Some(oauth_server()),
        ..TestServerOptions::default()
    })
    .await
}

#[expect(clippy::expect_used, reason = "test helper with a fixed time")]
fn now() -> DateTime {
    DateTime::try_from(NOW).expect("Invalid time")
}

#[expect(clippy::expect_used, reason = "test helper parsing fixed scopes")]
fn grant(server: &TestServer, user: &TestUser, user_key: Option<UserKeyUuid>) -> TestOAuthGrant {
    seed_oauth_grant(
        server,
        user,
        &"projects:read".parse().expect("Invalid scopes"),
        &oauth_resource(),
        user_key,
        now(),
    )
}

fn grant_path(user: &TestUser, grant: &TestOAuthGrant) -> String {
    format!("/v0/users/{}/oauth/grants/{}", user.slug, grant.uuid)
}

#[expect(clippy::expect_used, reason = "test helper creating a user key")]
async fn create_user_key(
    server: &TestServer,
    user: &TestUser,
    name: &str,
) -> (UserKeyUuid, UserKey) {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({"name": name}))
        .send()
        .await
        .expect("Failed to create a user key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let key: JsonUserKeyCreated = resp.json().await.expect("Invalid user key");
    (key.uuid, key.key)
}

/// Send a request with `bearer`, returning its status, its `X-Total-Count`, and its JSON body.
#[expect(clippy::expect_used, reason = "test helper")]
async fn request(
    server: &TestServer,
    method: Method,
    path: &str,
    bearer: &str,
) -> (StatusCode, Option<String>, Value) {
    let resp = server
        .client
        .request(method, server.api_url(path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let total_count = resp
        .headers()
        .get("X-Total-Count")
        .and_then(|count| count.to_str().ok())
        .map(ToOwned::to_owned);
    let body = resp.text().await.expect("Failed to read the response");
    (
        status,
        total_count,
        serde_json::from_str(&body).unwrap_or(Value::Null),
    )
}

/// Call a tool with the grant's access token and return the HTTP status.
#[expect(clippy::expect_used, reason = "test helper")]
async fn mcp_status(server: &TestServer, grant: &TestOAuthGrant) -> StatusCode {
    server
        .client
        .post(server.api_url("/mcp"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(grant.access_token.as_ref()),
        )
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/call",
            "params": {"name": "list_projects", "arguments": {}}
        }))
        .send()
        .await
        .expect("MCP request failed")
        .status()
}
