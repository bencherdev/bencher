#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for OIDC: ID tokens, the key set that verifies them, the user info
//! endpoint, discovery, and when a user's email counts as verified.

use bencher_api_tests::{
    TestServer, TestServerOptions, TestUser,
    helpers::{OAUTH_ISSUER, OAUTH_SIGNING_KEY, seed_token},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use bencher_json::{
    Clock, DateTime, JsonUserKeyCreated,
    system::config::{JsonOAuthServer, JsonRateLimiting},
};
use bencher_schema::{
    context::{OAuthServer, RateLimiting},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{Method, StatusCode, header::LOCATION};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::JwkSet};
use serde_json::{Value, json};
use url::{Url, form_urlencoded};

/// The time every test server here starts at.
const NOW: i64 = 1_800_000_000;
const CLIENT_ID: &str = "https://client.example/oauth/client.json";
const REDIRECT_URI: &str = "https://client.example/callback";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
/// The code verifier `CHALLENGE` was made from (RFC 7636, appendix B).
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const NONCE: &str = "n-0S6_WzA2Mj";
/// Every OIDC route, by method, and what its `OPTIONS` allows.
const OIDC_ROUTES: [(&str, &str); 3] = [
    ("/.well-known/openid-configuration", "GET"),
    ("/oauth/userinfo", "GET, POST"),
    ("/oauth/jwks.json", "GET"),
];

// Kills naming the user by their email, which they can change and someone else can then take,
// and kills telling more than the token's scopes allow.
#[tokio::test]
async fn userinfo_names_the_user_by_uuid() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Info", "info@example.com").await;

    let tokens = harness.tokens("openid email profile", &user.token).await;
    let access_token = string(&tokens, "access_token");
    let info = harness.userinfo_ok(&access_token).await;
    assert_eq!(
        info,
        json!({
            "sub": user.uuid,
            "email": user.email,
            "email_verified": true,
            "name": user.name,
            "preferred_username": user.slug,
        })
    );
    let resp = harness
        .client
        .post(harness.server.api_url("/oauth/userinfo"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&access_token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.json::<Value>().await.expect("Invalid user info"), info);

    let tokens = harness.tokens("openid", &user.token).await;
    let info = harness.userinfo_ok(&string(&tokens, "access_token")).await;
    assert_eq!(info, json!({ "sub": user.uuid }));
}

// Kills an email change that keeps the old address's proof, and an email link that is not
// counted as proof.
#[tokio::test]
async fn email_verified_follows_proof_of_the_address() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Mover", "mover@example.com").await;
    let tokens = harness.tokens("openid email", &user.token).await;
    let access_token = string(&tokens, "access_token");

    let resp = harness
        .server
        .client
        .patch(harness.server.api_url(&format!("/v0/users/{}", user.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({ "email": "moved@example.com" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let info = harness.userinfo_ok(&access_token).await;
    assert_eq!(info.get("email"), Some(&json!("moved@example.com")));
    assert_eq!(info.get("email_verified"), Some(&json!(false)));

    let token = harness
        .server
        .token_key()
        .new_auth("moved@example.com".parse().expect("Invalid email"), 60)
        .expect("Failed to mint an auth token");
    let resp = harness
        .server
        .client
        .post(harness.server.api_url("/v0/auth/confirm"))
        .json(&json!({ "token": token }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let info = harness.userinfo_ok(&access_token).await;
    assert_eq!(info.get("email_verified"), Some(&json!(true)));
}

// Kills counting any credential at consent as proof of the address: only a session is, since one
// is only minted for an address its user proved they hold, and only when it approves.
#[tokio::test]
async fn consent_verifies_the_email_only_with_a_session() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness
        .server
        .signup("Approver", "approver@example.com")
        .await;

    harness.unverify(&user);
    let request = harness.request("projects:read").await;
    assert_eq!(
        harness.answer(&request, &user.token).await.status(),
        StatusCode::OK
    );
    assert!(harness.email_verified(&user).is_some());

    harness.unverify(&user);
    assert_eq!(
        harness.answer(&request, &user.token).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        harness.email_verified(&user),
        None,
        "only an approval records the proof"
    );

    harness.unverify(&user);
    let key = harness.create_user_key(&user).await;
    harness.code("projects:read", &key).await;
    assert_eq!(harness.email_verified(&user), None);
}

// Kills approving with a legacy API token, which binds nothing and proves nothing.
#[tokio::test]
async fn api_token_cannot_approve() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Token", "token@example.com").await;
    harness.unverify(&user);
    let api_token = seed_token(&harness.server, &user, "Legacy");

    let request = harness.request("projects:read").await;
    let resp = harness.answer(&request, api_token.token.as_ref()).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp = harness.answer(&request, &user.token).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the request is still pending"
    );
    assert!(harness.email_verified(&user).is_some());
}

// Kills serving user info for a token that was not granted `openid`, or for no valid token.
#[tokio::test]
async fn userinfo_needs_an_access_token_with_openid() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Scoped", "scoped@example.com").await;
    let tokens = harness.tokens("projects:read email", &user.token).await;

    let resp = harness
        .userinfo(Some(&string(&tokens, "access_token")))
        .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        header(&resp, "www-authenticate"),
        r#"Bearer error="insufficient_scope", scope="openid""#
    );
    assert_eq!(
        header(&resp, "access-control-expose-headers"),
        "WWW-Authenticate"
    );

    let tokens = harness.tokens("openid", &user.token).await;
    for token in [
        Some(string(&tokens, "refresh_token")),
        Some("bencher_oauth_000000000000000000000000000000".to_owned()),
        Some(user.token.clone()),
    ] {
        let resp = harness.userinfo(token.as_deref()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(
            header(&resp, "www-authenticate").starts_with(r#"Bearer error="invalid_token""#),
            "{token:?}"
        );
    }
    let resp = harness.userinfo(None).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(header(&resp, "www-authenticate"), "Bearer");
}

// Kills an ID token a client cannot verify with the advertised key set, one meant for another
// audience, and one that drops the client's nonce.
#[tokio::test]
async fn id_token_verifies_against_the_advertised_key_set() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Signer", "signer@example.com").await;
    let code = harness.code("openid email", &user.token).await;
    harness.advance(60);
    let tokens = harness.exchange(&code).await;
    let id_token = string(&tokens, "id_token");

    let configuration = harness.get_json("/.well-known/openid-configuration").await;
    let jwks_uri = string(&configuration, "jwks_uri");
    let jwks_path = jwks_uri
        .strip_prefix(OAUTH_ISSUER)
        .expect("Key set not on the issuer");
    let jwk_set: JwkSet =
        serde_json::from_value(harness.get_json(jwks_path).await).expect("Invalid key set");
    let header = jsonwebtoken::decode_header(&id_token).expect("Invalid ID token header");
    let jwk = jwk_set
        .find(header.kid.as_deref().expect("Missing key ID"))
        .expect("Unknown key ID");
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[CLIENT_ID]);
    validation.set_issuer(&[OAUTH_ISSUER]);
    let claims = jsonwebtoken::decode::<Value>(
        &id_token,
        &DecodingKey::from_jwk(jwk).expect("Invalid key"),
        &validation,
    )
    .expect("ID token does not verify")
    .claims;
    assert_eq!(claims.get("sub"), Some(&json!(user.uuid)));
    assert_eq!(claims.get("nonce"), Some(&json!(NONCE)));
    assert_eq!(claims.get("email"), Some(&json!(user.email)));
    assert_eq!(claims.get("email_verified"), Some(&json!(true)));
    assert_eq!(claims.get("name"), None, "profile was not granted");
    assert_eq!(integer(&claims, "auth_time"), NOW, "approved at");
    assert_eq!(integer(&claims, "iat"), NOW + 60, "exchanged at");
    assert_eq!(integer(&claims, "exp"), NOW + 60 + 10 * 60);

    let tokens = harness.tokens("projects:read", &user.token).await;
    assert_eq!(tokens.get("id_token"), None);
}

// Kills advertising or granting OpenID Connect without a key to sign ID tokens with.
#[tokio::test]
async fn openid_connect_needs_a_signing_key() {
    let server = TestServer::new().await;
    let harness = Harness::new(SigningKey::Absent).await;
    for (client, api_url) in [
        (&server.client, server.api_url("")),
        (&harness.client, harness.server.api_url("")),
    ] {
        for (path, methods) in OIDC_ROUTES {
            for method in methods.split(", ").chain(["OPTIONS"]) {
                let method = Method::from_bytes(method.as_bytes()).expect("Invalid method");
                let resp = client
                    .request(method.clone(), format!("{api_url}{path}"))
                    .send()
                    .await
                    .expect("Request failed");
                assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} {path}");
            }
        }
    }
    let metadata = harness
        .get_json("/.well-known/oauth-authorization-server")
        .await;
    assert_eq!(
        metadata.get("scopes_supported"),
        Some(&json!([
            "projects:read",
            "projects:write",
            "offline_access"
        ]))
    );
    assert_eq!(metadata.get("userinfo_endpoint"), None);
    let user = harness
        .server
        .signup("Keyless", "keyless@example.com")
        .await;
    let tokens = harness.tokens("openid email", &user.token).await;
    assert_eq!(tokens.get("scope"), Some(&json!("projects:read")));
    assert_eq!(tokens.get("id_token"), None);
    let resp = harness
        .token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &string(&tokens, "refresh_token")),
            ("client_id", CLIENT_ID),
            ("scope", "openid projects:read"),
        ])
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "an identity scope is ignored"
    );
}

// Kills an OIDC route or field missing while ID tokens are signed, and an identity scope lost at
// refresh.
#[tokio::test]
async fn openid_connect_with_a_signing_key() {
    let harness = Harness::new(SigningKey::Configured).await;
    for (path, methods) in OIDC_ROUTES {
        let resp = harness
            .client
            .request(Method::OPTIONS, harness.server.api_url(path))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&resp, "access-control-allow-origin"), "*");
        assert_eq!(header(&resp, "access-control-allow-methods"), methods);
    }
    let metadata = harness
        .get_json("/.well-known/oauth-authorization-server")
        .await;
    assert_eq!(
        harness.get_json("/.well-known/openid-configuration").await,
        metadata
    );
    assert_eq!(
        metadata.get("scopes_supported"),
        Some(&json!([
            "projects:read",
            "projects:write",
            "openid",
            "email",
            "profile",
            "offline_access"
        ]))
    );
    for (field, path) in [
        ("userinfo_endpoint", "/oauth/userinfo"),
        ("jwks_uri", "/oauth/jwks.json"),
    ] {
        assert_eq!(
            metadata.get(field),
            Some(&json!(format!("{OAUTH_ISSUER}{path}")))
        );
    }
    assert_eq!(
        metadata.get("id_token_signing_alg_values_supported"),
        Some(&json!(["RS256"]))
    );
    let user = harness.server.signup("Keyed", "keyed@example.com").await;
    let tokens = harness.tokens("openid", &user.token).await;
    assert_eq!(
        tokens.get("scope"),
        Some(&json!("openid projects:read")),
        "a grant can always use the MCP server"
    );
    let resp = harness
        .token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &string(&tokens, "refresh_token")),
            ("client_id", CLIENT_ID),
            ("scope", "openid"),
        ])
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let refreshed: Value = resp.json().await.expect("Invalid token response");
    assert_eq!(refreshed.get("scope"), Some(&json!("openid projects:read")));
}

// Kills user info that is not charged to its user, and refused tokens that are.
#[tokio::test]
async fn userinfo_is_charged_to_its_user() {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"user": {"requests": {"minute": 8}}}))
            .expect("Invalid rate limiting");
    let harness = Harness::with_rate_limiting(
        SigningKey::Configured,
        Some(RateLimiting::from(rate_limiting)),
    )
    .await;
    let mut answered = Vec::new();
    for (name, refused) in [("Plain", 0), ("Refused", 4)] {
        let user = harness
            .server
            .signup(name, &format!("{}@example.com", name.to_lowercase()))
            .await;
        let tokens = harness.tokens("openid", &user.token).await;
        let refresh_token = string(&tokens, "refresh_token");
        for _ in 0..refused {
            let resp = harness.userinfo(Some(&refresh_token)).await;
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }
        let access_token = string(&tokens, "access_token");
        let mut ok = 0;
        loop {
            let resp = harness.userinfo(Some(&access_token)).await;
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                assert_eq!(header(&resp, "retry-after"), "60");
                assert_eq!(header(&resp, "access-control-allow-origin"), "*");
                break;
            }
            assert_eq!(resp.status(), StatusCode::OK);
            ok += 1;
            assert!(ok < 8, "user info is never limited");
        }
        answered.push(ok);
    }
    assert!(answered[0] > 0);
    assert_eq!(answered[0], answered[1], "refused tokens are not charged");
}

// Kills an unchanged email written back from a stale read, which would carry a newer address's
// proof onto it.
#[tokio::test]
async fn unchanged_email_is_never_written_back() {
    let harness = Harness::new(SigningKey::Configured).await;
    let user = harness.server.signup("Stale", "stale@example.com").await;
    let mut writer = Arc::clone(&harness.server.context().database.connection)
        .lock_owned()
        .await;
    let patch = harness
        .server
        .client
        .patch(harness.server.api_url(&format!("/v0/users/{}", user.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({ "email": "stale@example.com", "name": "Renamed" }))
        .send();
    let (resp, ()) = tokio::join!(patch, async move {
        // Once the request has read the user, the address moves and is proven elsewhere.
        tokio::time::sleep(Duration::from_secs(1)).await;
        diesel::update(schema::user::table.filter(schema::user::uuid.eq(&user.uuid)))
            .set((
                schema::user::email.eq("proven@example.com"),
                schema::user::email_verified.eq(DateTime::try_from(NOW).expect("Invalid time")),
            ))
            .execute(&mut *writer)
            .expect("Failed to move the address");
        drop(writer);
    });
    assert_eq!(resp.expect("Request failed").status(), StatusCode::OK);
    let (email, email_verified): (String, Option<DateTime>) = schema::user::table
        .select((schema::user::email, schema::user::email_verified))
        .first(&mut harness.server.db_conn())
        .expect("Missing user");
    assert_eq!(email, "proven@example.com");
    assert!(email_verified.is_some());
}

#[derive(Clone, Copy)]
enum SigningKey {
    Configured,
    Absent,
}

struct Harness {
    server: TestServer,
    /// A client that does not follow redirects, so every `Location` can be read.
    client: reqwest::Client,
    now: Arc<AtomicI64>,
}

impl Harness {
    async fn new(signing_key: SigningKey) -> Self {
        Self::with_rate_limiting(signing_key, None).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn with_rate_limiting(
        signing_key: SigningKey,
        rate_limiting: Option<RateLimiting>,
    ) -> Self {
        let signing_key = match signing_key {
            SigningKey::Configured => Some(OAUTH_SIGNING_KEY),
            SigningKey::Absent => None,
        };
        let config = json!({
            "issuer": OAUTH_ISSUER,
            "signing_key": signing_key,
            "clients": [{
                "id": CLIENT_ID,
                "metadata": {
                    "client_id": CLIENT_ID,
                    "redirect_uris": [REDIRECT_URI],
                    "grant_types": ["authorization_code", "refresh_token"],
                },
            }],
        });
        let json: JsonOAuthServer = serde_json::from_value(config).expect("Invalid OAuth config");
        let now = Arc::new(AtomicI64::new(NOW));
        let clock_now = Arc::clone(&now);
        let server = TestServer::new_with_options(TestServerOptions {
            clock: Some(Clock::Custom(Arc::new(move || {
                DateTime::try_from(clock_now.load(Ordering::SeqCst)).expect("Invalid time")
            }))),
            rate_limiting,
            oauth: Some(OAuthServer::try_from(json).expect("Invalid OAuth server")),
            ..TestServerOptions::default()
        })
        .await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build a client");
        Self {
            server,
            client,
            now,
        }
    }

    fn advance(&self, seconds: i64) {
        self.now.fetch_add(seconds, Ordering::SeqCst);
    }

    /// A pending request asking for `scope`, by its UUID.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn request(&self, scope: &str) -> String {
        let resp = self
            .client
            .get(self.server.api_url("/oauth/authorize"))
            .query(&[
                ("response_type", "code"),
                ("client_id", CLIENT_ID),
                ("redirect_uri", REDIRECT_URI),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
                ("scope", scope),
                ("nonce", NONCE),
            ])
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FOUND);
        let location = Url::parse(header(&resp, LOCATION.as_str())).expect("Invalid Location");
        param(&location, "request")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn answer(&self, request: &str, bearer: &str) -> reqwest::Response {
        self.client
            .post(
                self.server
                    .api_url(&format!("/v0/oauth/authorizations/{request}")),
            )
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .json(&json!({ "approve": true }))
            .send()
            .await
            .expect("Request failed")
    }

    /// An authorization code for `scope`, approved with `bearer`.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn code(&self, scope: &str, bearer: &str) -> String {
        let request = self.request(scope).await;
        let resp = self.answer(&request, bearer).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let answer: Value = resp.json().await.expect("Invalid answer");
        let url = Url::parse(&string(&answer, "url")).expect("Invalid answer URL");
        param(&url, "code")
    }

    /// The token response for a grant of `scope` the user approved with a session.
    async fn tokens(&self, scope: &str, session: &str) -> Value {
        let code = self.code(scope, session).await;
        self.exchange(&code).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn exchange(&self, code: &str) -> Value {
        let resp = self
            .token(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", CLIENT_ID),
                ("redirect_uri", REDIRECT_URI),
                ("code_verifier", VERIFIER),
            ])
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        resp.json().await.expect("Invalid token response")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn token(&self, params: &[(&str, &str)]) -> reqwest::Response {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();
        self.client
            .post(self.server.api_url("/oauth/token"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn userinfo(&self, access_token: Option<&str>) -> reqwest::Response {
        let mut request = self.client.get(self.server.api_url("/oauth/userinfo"));
        if let Some(access_token) = access_token {
            request = request.header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(access_token),
            );
        }
        request.send().await.expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn userinfo_ok(&self, access_token: &str) -> Value {
        let resp = self.userinfo(Some(access_token)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "cache-control"), "no-store");
        resp.json().await.expect("Invalid user info")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(self.server.api_url(path))
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn get_json(&self, path: &str) -> Value {
        let resp = self.get(path).await;
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&resp, "access-control-allow-origin"), "*");
        resp.json().await.expect("Invalid JSON")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn create_user_key(&self, user: &TestUser) -> String {
        let resp = self
            .server
            .client
            .post(
                self.server
                    .api_url(&format!("/v0/users/{}/keys", user.slug)),
            )
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&json!({ "name": "Pasted Key" }))
            .send()
            .await
            .expect("Failed to create a user key");
        assert_eq!(resp.status(), StatusCode::CREATED);
        let key: JsonUserKeyCreated = resp.json().await.expect("Invalid user key");
        key.key.to_string()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn email_verified(&self, user: &TestUser) -> Option<DateTime> {
        schema::user::table
            .filter(schema::user::uuid.eq(&user.uuid))
            .select(schema::user::email_verified)
            .first(&mut self.server.db_conn())
            .expect("Missing user")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn unverify(&self, user: &TestUser) {
        diesel::update(schema::user::table.filter(schema::user::uuid.eq(&user.uuid)))
            .set(schema::user::email_verified.eq(None::<DateTime>))
            .execute(&mut self.server.db_conn())
            .expect("Failed to unverify the user");
    }
}

#[expect(clippy::expect_used, reason = "test helper")]
fn header<'r>(resp: &'r reqwest::Response, name: &str) -> &'r str {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .expect("Missing header")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn param(url: &Url, name: &str) -> String {
    url.query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
        .expect("Missing parameter")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn string(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .expect("Missing string")
        .to_owned()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn integer(value: &Value, name: &str) -> i64 {
    value
        .get(name)
        .and_then(Value::as_i64)
        .expect("Missing integer")
}
