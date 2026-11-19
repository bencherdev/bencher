#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for presented credentials that fail: each is one 401 with one message,
//! on a path that needs a credential and on a public one alike, and none reaches a response
//! or a log line.

use bencher_api_tests::{
    LogCapture, TestServer, TestServerOptions, TestUser,
    helpers::{oauth_server, seed_token, seed_token_jwt},
    session_cookie,
};
use bencher_json::{DateTime, Email, Jwt, ProjectKey, UserKey, UserKeyHash};
use bencher_schema::{error::INVALID_CREDENTIAL, schema};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use http::{HeaderValue, StatusCode, header};
use serde_json::json;

const UNKNOWN_USER_KEY: &str = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
const UNKNOWN_PROJECT_KEY: &str = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";

// Kills any failing credential answered with another status or message, on any kind of path,
// a malformed one read as anonymous on a public path, a dead cookie that is not read as
// anonymous there, or a project key refused by a public path the CLI reads before a run.
#[tokio::test]
async fn failing_credentials_are_one_unauthorized() {
    let server = TestServer::new().await;
    let fixture = Fixture::new(&server).await;

    for path in fixture.paths() {
        for (case, value) in fixture.failing() {
            let resp = request(&server, &path, Some(&value)).await;
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{case} at {path}");
            let body = resp.text().await.expect("Failed to read the body");
            let json: serde_json::Value = serde_json::from_str(&body).expect("{body}");
            assert_eq!(json["message"], INVALID_CREDENTIAL, "{case} at {path}");
        }

        let key = bearer(fixture.key.as_ref());
        assert_eq!(
            request(&server, &path, Some(&key)).await.status(),
            StatusCode::OK,
            "{path}"
        );
    }

    let project_key = bearer(fixture.project_key.as_ref());
    for (path, status) in [
        (&fixture.user_path, StatusCode::UNAUTHORIZED),
        (&fixture.project_path, StatusCode::OK),
        (&fixture.console_path, StatusCode::OK),
    ] {
        let resp = request(&server, path, Some(&project_key)).await;
        assert_eq!(resp.status(), status, "{path}");
    }

    for (case, value) in fixture.failing() {
        let resp = mcp(&server, &value).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{case} at /mcp");
        let json: serde_json::Value = resp.json().await.expect("Failed to parse the error");
        assert_eq!(json["message"], INVALID_CREDENTIAL, "{case} at /mcp");
    }

    let dead = dead_cookie();
    let resp = with_cookie(&server, &fixture.user_path, &dead).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json: serde_json::Value = resp.json().await.expect("Failed to parse the error");
    assert_eq!(json["message"], INVALID_CREDENTIAL);
    for path in [&fixture.project_path, &fixture.console_path] {
        assert_eq!(request(&server, path, None).await.status(), StatusCode::OK);
        assert_eq!(
            with_cookie(&server, path, &dead).await.status(),
            StatusCode::OK,
            "{path}"
        );
    }
}

// Kills an OAuth access token at `/mcp` that fails with any other message.
#[tokio::test]
async fn failing_oauth_token_is_the_same_unauthorized() {
    let server = TestServer::new_with_options(TestServerOptions {
        oauth: Some(oauth_server()),
        ..TestServerOptions::default()
    })
    .await;
    let resp = mcp(
        &server,
        &bearer("bencher_oauth_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json: serde_json::Value = resp.json().await.expect("Failed to parse the error");
    assert_eq!(json["message"], INVALID_CREDENTIAL);
}

// Kills a presented credential, or any part of a JWT, echoed into a response or a log line,
// whether it fails at REST, at `/mcp`, or at the registry, or a live one meets a failing database
// in its lookup or in the 500 after it.
#[tokio::test]
async fn no_presented_credential_reaches_a_response_or_a_log() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let fixture = Fixture::new(&server).await;

    let mut seen = Vec::new();
    for (_, value) in fixture.failing() {
        for path in fixture.paths() {
            seen.push(response_text(request(&server, &path, Some(&value)).await).await);
        }
        seen.push(response_text(mcp(&server, &value).await).await);
    }
    for (_, secret) in fixture.secrets() {
        let resp = server
            .client
            .get(server.api_url("/v0/auth/oci/token?scope=repository:x:push"))
            .basic_auth(fixture.user.email.as_ref(), Some(&secret))
            .send()
            .await
            .expect("Request failed");
        seen.push(response_text(resp).await);
    }

    let live = bearer(fixture.live_legacy.as_ref());
    for (table, status) in [
        ("organization_role", StatusCode::INTERNAL_SERVER_ERROR),
        ("token", StatusCode::UNAUTHORIZED),
    ] {
        let conn = &mut server.db_conn();
        conn.batch_execute(&format!("ALTER TABLE {table} RENAME TO {table}_gone"))
            .expect("Failed to break the table");
        let resp = request(&server, &fixture.user_path, Some(&live)).await;
        assert_eq!(resp.status(), status, "{table}");
        seen.push(response_text(resp).await);
        conn.batch_execute(&format!("ALTER TABLE {table}_gone RENAME TO {table}"))
            .expect("Failed to mend the table");
    }

    let lines = capture.lines();
    let live = ("live token", fixture.live_legacy.as_ref().to_owned());
    for (_, secret) in fixture.secrets().into_iter().chain([live]) {
        for part in parts(&secret) {
            for text in &seen {
                assert!(!text.contains(part), "{secret} in a response: {text}");
            }
            for line in &lines {
                assert!(!line.contains(part), "{secret} in a log line: {line}");
            }
        }
    }
}

struct Fixture {
    user: TestUser,
    user_path: String,
    project_path: String,
    console_path: String,
    key: UserKey,
    project_key: ProjectKey,
    revoked_key: UserKey,
    expired_key: UserKey,
    locked_key: UserKey,
    rowless_legacy: Jwt,
    revoked_legacy: Jwt,
    expired_legacy: Jwt,
    locked_legacy: Jwt,
    live_legacy: Jwt,
    unsigned_session: Jwt,
    unknown_session: Jwt,
    locked_session: String,
}

impl Fixture {
    async fn new(server: &TestServer) -> Self {
        let _admin = server.signup("Root", "root-credentials@example.com").await;
        let user = server.signup("Some User", "some-user@example.com").await;
        let locked = server
            .signup("Locked User", "locked-user@example.com")
            .await;
        let org = server.create_org(&user, "Public Org").await;
        let project = server.create_project(&user, &org, "Public Project").await;

        let key = server.create_user_key(&user, "live").await;
        let project_key = server.create_project_key(&user, &project, "live").await.key;
        let revoked_key = server.create_user_key(&user, "revoked").await;
        set_user_key(
            server,
            &revoked_key,
            schema::user_key::revoked.eq(Some(now(server))),
        );
        let expired_key = server.create_user_key(&user, "expired").await;
        set_user_key(
            server,
            &expired_key,
            schema::user_key::expiration.eq(now(server)),
        );
        let locked_key = server.create_user_key(&locked, "locked").await;

        let rowless_legacy = mint(server, &user.email, 1);
        let revoked = seed_token(server, &user, "revoked");
        diesel::update(schema::token::table.filter(schema::token::uuid.eq(revoked.uuid)))
            .set(schema::token::revoked.eq(Some(now(server))))
            .execute(&mut server.db_conn())
            .expect("Failed to revoke the token");
        let expired_legacy = mint(server, &user.email, 2);
        seed_token_jwt(server, &user, "expired", &expired_legacy, now(server));
        let locked_legacy = seed_token(server, &locked, "locked").token;
        let live_legacy = seed_token(server, &user, "live").token;

        let session: Jwt = user.token.parse().expect("Invalid session");
        let other = mint(server, &user.email, 3);
        let unsigned_session = with_signature_of(&session, &other);
        let unknown_session = server
            .token_key()
            .new_client(
                "nobody@example.com".parse().expect("Invalid email"),
                u32::MAX,
            )
            .expect("Failed to mint a session");
        let locked_session = locked.token.clone();

        diesel::update(schema::user::table.filter(schema::user::uuid.eq(locked.uuid)))
            .set(schema::user::locked.eq(true))
            .execute(&mut server.db_conn())
            .expect("Failed to lock the user");

        Self {
            user_path: format!("/v0/users/{}", user.slug),
            project_path: format!("/v0/projects/{}", project.slug),
            console_path: "/v0/server/config/console".to_owned(),
            user,
            key,
            project_key,
            revoked_key,
            expired_key,
            locked_key,
            rowless_legacy,
            revoked_legacy: revoked.token,
            expired_legacy,
            locked_legacy,
            live_legacy,
            unsigned_session,
            unknown_session,
            locked_session,
        }
    }

    /// A path that needs a credential, a public project, and a public path that reads none.
    fn paths(&self) -> [String; 3] {
        [
            self.user_path.clone(),
            self.project_path.clone(),
            self.console_path.clone(),
        ]
    }

    /// Every secret a failing credential presents, by what makes it fail.
    fn secrets(&self) -> Vec<(&'static str, String)> {
        vec![
            ("malformed", "not-a-credential".to_owned()),
            ("malformed user key", "bencher_user_short".to_owned()),
            ("unknown user key", UNKNOWN_USER_KEY.to_owned()),
            ("revoked user key", self.revoked_key.as_ref().to_owned()),
            ("expired user key", self.expired_key.as_ref().to_owned()),
            ("locked user key", self.locked_key.as_ref().to_owned()),
            ("unknown project key", UNKNOWN_PROJECT_KEY.to_owned()),
            ("rowless token", self.rowless_legacy.as_ref().to_owned()),
            ("revoked token", self.revoked_legacy.as_ref().to_owned()),
            ("expired token", self.expired_legacy.as_ref().to_owned()),
            ("locked token", self.locked_legacy.as_ref().to_owned()),
            (
                "unsigned session",
                self.unsigned_session.as_ref().to_owned(),
            ),
            ("unknown session", self.unknown_session.as_ref().to_owned()),
            ("locked session", self.locked_session.clone()),
        ]
    }

    /// Every failing `Authorization` header, by what makes it fail.
    fn failing(&self) -> Vec<(&'static str, HeaderValue)> {
        let mut failing: Vec<(&'static str, HeaderValue)> = self
            .secrets()
            .into_iter()
            .map(|(case, secret)| (case, bearer(&secret)))
            .collect();
        failing.extend([
            (
                "another scheme",
                HeaderValue::from_static("Basic c29tZTp1c2Vy"),
            ),
            ("no token", HeaderValue::from_static("Bearer ")),
            (
                "not text",
                HeaderValue::from_bytes(b"Bearer \xff\xfe").expect("Invalid header"),
            ),
        ]);
        failing
    }
}

/// A cookie that holds a well formed session token no session has.
fn dead_cookie() -> String {
    session_cookie(&"A".repeat(43).parse().expect("Invalid session token"))
}

async fn request(
    server: &TestServer,
    path: &str,
    authorization: Option<&HeaderValue>,
) -> reqwest::Response {
    let mut request = server.client.get(server.api_url(path));
    if let Some(authorization) = authorization {
        request = request.header(header::AUTHORIZATION, authorization.clone());
    }
    request.send().await.expect("Request failed")
}

async fn mcp(server: &TestServer, authorization: &HeaderValue) -> reqwest::Response {
    server
        .client
        .post(server.api_url("/mcp"))
        .header(header::AUTHORIZATION, authorization.clone())
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .send()
        .await
        .expect("Request failed")
}

async fn with_cookie(server: &TestServer, path: &str, cookie: &str) -> reqwest::Response {
    server
        .client
        .get(server.api_url(path))
        .header(header::COOKIE, cookie)
        .send()
        .await
        .expect("Request failed")
}

/// The response's headers and body, as one text to search.
async fn response_text(resp: reqwest::Response) -> String {
    let headers = format!("{:?}", resp.headers());
    let body = resp.text().await.expect("Failed to read the body");
    format!("{headers}\n{body}")
}

fn bearer(secret: &str) -> HeaderValue {
    HeaderValue::from_str(&bencher_json::bearer_header(secret)).expect("Invalid header")
}

/// What of a secret must never be seen: all of it, and for a JWT each part that is its own.
fn parts(secret: &str) -> Vec<&str> {
    let mut parts = vec![secret];
    if let Some((_, rest)) = secret.split_once('.')
        && let Some((payload, signature)) = rest.split_once('.')
    {
        parts.extend([payload, signature]);
    }
    parts
}

fn now(server: &TestServer) -> DateTime {
    server.context().clock.now()
}

/// A legacy token for `email`, made distinct from the others by `nonce`.
fn mint(server: &TestServer, email: &Email, nonce: u32) -> Jwt {
    server
        .token_key()
        .new_api_key(email.clone(), u32::MAX - nonce)
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

fn set_user_key<V>(server: &TestServer, key: &UserKey, value: V)
where
    V: diesel::query_builder::AsChangeset<Target = schema::user_key::table>,
    V::Changeset: diesel::query_builder::QueryFragment<diesel::sqlite::Sqlite>,
{
    let hash = UserKeyHash::from(key);
    let rows = diesel::update(
        schema::user_key::table.filter(schema::user_key::key_hash.eq(hash.as_ref())),
    )
    .set(value)
    .execute(&mut server.db_conn())
    .expect("Failed to update the user key");
    assert_eq!(rows, 1);
}
