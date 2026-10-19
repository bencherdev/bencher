#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the OAuth authorization server: the authorization endpoint, the consent
//! API, the token and revocation endpoints, and the discovery documents.

use std::{
    collections::HashMap,
    io,
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use bencher_api_tests::{
    TestServer, TestServerOptions, TestUser,
    helpers::{OAUTH_ISSUER, oauth_resource, seed_token},
};
use bencher_egress::{EgressClient, EgressPolicy, EgressTimeouts, Lookup};
use bencher_json::{
    Clock, DateTime, JsonOAuthAuthorization, JsonOAuthRedirect, JsonUserKeyCreated,
    OAuthAccessToken, OAuthAuthorizationUuid, OAuthCode, OAuthCodeHash, OAuthRefreshToken,
    OAuthScope, OAuthTokenHash, UserKey, UserKeyUuid,
    system::config::{JsonOAuthServer, JsonRateLimiting},
};
use bencher_schema::{
    context::{DbConnection, OAuthServer, RateLimiting},
    model::user::{
        UserId,
        key::UserKeyId,
        oauth::{OAuthTokenKind, QueryOAuthGrant, QueryOAuthToken},
        oauth_authorization::{NewOAuthAuthorization, OAuthAnswer, QueryOAuthAuthorization},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use http::{
    Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST, LOCATION},
};
use rustls::crypto::aws_lc_rs;
use serde_json::{Value, json};
use url::{Url, form_urlencoded};

/// The time every test server here starts at.
const NOW: i64 = 1_800_000_000;
/// Far longer than a request takes to pass its read path and wait on the writer.
const READ_PATH: Duration = Duration::from_secs(1);
const CONSOLE_URL: &str = "http://localhost:3000";
const CLIENT_ID: &str = "https://client.example/oauth/client.json";
const REDIRECT_URI: &str = "https://client.example/callback";
const LOOPBACK_CLIENT_ID: &str = "https://cli.example/oauth/client.json";
/// Configured without a document, so it is fetched.
const FETCHED_CLIENT_ID: &str = "https://fetched.example/oauth/client.json";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const SHORT_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-c";
const STATE: &str = "af0ifjsldkj";
/// The code verifier `CHALLENGE` was made from (RFC 7636, appendix B).
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const OTHER_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXl";

#[tokio::test]
async fn approve_with_a_session() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Approver", "approver@example.com")
        .await;

    let uuid = consent_request(&harness.authorize(&request()).await);
    let resp = harness.view(uuid).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let authorization: JsonOAuthAuthorization = resp.json().await.expect("Invalid authorization");
    assert_eq!(authorization.uuid, uuid);
    assert_eq!(authorization.client.id.as_ref(), CLIENT_ID);
    assert_eq!(authorization.client.host, "client.example");
    assert_eq!(authorization.client.name.as_ref(), "Example Client");
    assert!(authorization.client.verified);
    assert_eq!(authorization.redirect.host, "client.example");
    assert!(!authorization.redirect.loopback);
    assert_eq!(
        authorization.scopes,
        vec![OAuthScope::ProjectsRead, OAuthScope::ProjectsWrite]
    );
    assert_eq!(authorization.expiration.timestamp(), NOW + 30 * 60);

    let resp = harness.answer(uuid, true, Some(&user.token)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    assert_eq!(header(&resp, "access-control-allow-origin"), "*");
    let url = redirect_url(resp).await;
    assert!(
        url.as_str().starts_with(&format!("{REDIRECT_URI}?")),
        "{url}"
    );
    let params = query(&url);
    let code: OAuthCode = param(&params, "code").parse().expect("Invalid code");
    assert_eq!(param(&params, "state"), STATE);
    assert_eq!(param(&params, "iss"), OAUTH_ISSUER);
    assert_eq!(params.len(), 3, "{url}");

    let authorization = harness.authorization(uuid);
    assert_eq!(authorization.code_hash, Some(OAuthCodeHash::from(&code)));
    assert_eq!(authorization.user_id, Some(harness.user_id(&user)));
    assert_eq!(authorization.user_key_id, None);
    assert_eq!(authorization.expiration.timestamp(), NOW + 5 * 60);
}

// What a request asks for is recorded as this server understands it.
#[tokio::test]
async fn request_is_recorded_normalized() {
    let harness = Harness::new().await;
    let nonce = "n".repeat(256);
    for (changes, scope, resource) in [
        (
            vec![
                ("scope", Some("openid projects:write offline_access bogus")),
                ("nonce", Some(nonce.as_str())),
            ],
            "projects:write",
            format!("{OAUTH_ISSUER}/mcp"),
        ),
        (
            vec![("scope", None)],
            "projects:read",
            format!("{OAUTH_ISSUER}/mcp"),
        ),
        (
            vec![
                ("scope", Some("")),
                ("resource", Some("https://MCP.example.com/mcp/")),
            ],
            "projects:read",
            "https://mcp.example.com/mcp".to_owned(),
        ),
    ] {
        let uuid = consent_request(&harness.authorize(&request_with(&changes)).await);
        let authorization = harness.authorization(uuid);
        assert_eq!(authorization.scope.to_string(), scope);
        assert_eq!(authorization.resource.as_ref(), resource);
        assert_eq!(authorization.client_id.as_ref(), CLIENT_ID);
        assert_eq!(authorization.client_name.as_ref(), "Example Client");
        assert_eq!(authorization.redirect_uri.as_ref(), REDIRECT_URI);
        assert_eq!(authorization.code_challenge.as_ref(), CHALLENGE);
        assert_eq!(authorization.state.as_ref().map(AsRef::as_ref), Some(STATE));
        assert!(authorization.refresh);
        assert!(!authorization.is_answered());
    }
    let uuid = consent_request(
        &harness
            .authorize(&request_with(&[("nonce", Some(&nonce))]))
            .await,
    );
    assert_eq!(
        harness
            .authorization(uuid)
            .nonce
            .as_ref()
            .map(AsRef::as_ref),
        Some(nonce.as_str())
    );

    let uuid = consent_request(
        &harness
            .authorize(&loopback_request("http://localhost:53682/callback"))
            .await,
    );
    let authorization = harness.authorization(uuid);
    assert!(!authorization.refresh);
    assert_eq!(authorization.client_name.as_ref(), "cli.example");
}

// A client is sent nothing until both it and the redirect URI it asked for are known good.
#[tokio::test]
async fn untrusted_redirect_uri_is_never_followed() {
    let harness = Harness::new().await;
    for redirect_uri in [
        "https://evil.example/callback",
        "https://client.example/callback/",
        "https://client.example/callback/more",
        "https://client.example/callbacks",
        "https://client.example/callback?next=1",
        "https://client.example.evil.example/callback",
        "https://client.example:443/callback",
        "http://client.example/callback",
        "not a url",
    ] {
        let resp = harness
            .authorize(&request_with(&[("redirect_uri", Some(redirect_uri))]))
            .await;
        assert_eq!(
            location(&resp),
            error_page("invalid_redirect_uri"),
            "{redirect_uri}"
        );
    }
    for client_id in [
        "http://client.example/oauth/client.json",
        "https://203.0.113.7/oauth/client.json",
        "https://client.example/oauth/client.json/",
        "https://unlisted.example/oauth/client.json",
    ] {
        let resp = harness
            .authorize(&request_with(&[("client_id", Some(client_id))]))
            .await;
        assert_eq!(location(&resp), error_page("invalid_client"), "{client_id}");
    }
    for missing in ["client_id", "redirect_uri"] {
        let resp = harness.authorize(&request_with(&[(missing, None)])).await;
        assert_eq!(location(&resp), error_page("invalid_request"), "{missing}");
    }
    assert_eq!(harness.authorizations(), 0);
}

#[tokio::test]
async fn loopback_redirect_uri_on_any_port() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Loopback", "loopback@example.com")
        .await;
    for redirect_uri in [
        "http://localhost:53682/callback",
        "http://127.0.0.1:1/callback",
        "http://[::1]:8080/callback",
    ] {
        let uuid = consent_request(&harness.authorize(&loopback_request(redirect_uri)).await);
        let resp = harness.view(uuid).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let authorization: JsonOAuthAuthorization =
            resp.json().await.expect("Invalid authorization");
        assert!(authorization.redirect.loopback, "{redirect_uri}");

        let resp = harness.answer(uuid, true, Some(&user.token)).await;
        let url = redirect_url(resp).await;
        assert!(
            url.as_str().starts_with(&format!("{redirect_uri}?code=")),
            "{url}"
        );
    }
    for redirect_uri in [
        "http://localhost.evil.example/callback",
        "http://localhost:53682/other",
        "http://localhost:53682/callback?next=1",
        "https://localhost:53682/callback",
        "http://127.0.0.2:53682/callback",
    ] {
        let resp = harness.authorize(&loopback_request(redirect_uri)).await;
        assert_eq!(
            location(&resp),
            error_page("invalid_redirect_uri"),
            "{redirect_uri}"
        );
    }
}

#[tokio::test]
async fn registered_query_is_kept() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Query", "query@example.com").await;
    let redirect_uri = "https://client.example/callback?tenant=1";
    let uuid = consent_request(
        &harness
            .authorize(&request_with(&[("redirect_uri", Some(redirect_uri))]))
            .await,
    );
    let url = redirect_url(harness.answer(uuid, true, Some(&user.token)).await).await;
    assert!(
        url.as_str().starts_with(&format!("{redirect_uri}&code=")),
        "{url}"
    );
}

// Once the redirect is trusted, every refusal goes back to the client with its state and the issuer.
#[tokio::test]
async fn refusal_returns_state_and_iss() {
    let harness = Harness::new().await;
    let nonce = "n".repeat(257);
    for (changes, error) in [
        (
            vec![("code_challenge_method", Some("plain"))],
            "invalid_request",
        ),
        (vec![("code_challenge_method", None)], "invalid_request"),
        (vec![("code_challenge", None)], "invalid_request"),
        (
            vec![("code_challenge", Some(SHORT_CHALLENGE))],
            "invalid_request",
        ),
        (
            vec![("response_type", Some("token"))],
            "unsupported_response_type",
        ),
        (vec![("response_type", None)], "invalid_request"),
        (
            vec![("resource", Some("https://evil.example/mcp"))],
            "invalid_target",
        ),
        (vec![("prompt", Some("none"))], "consent_required"),
        (vec![("prompt", Some("login none"))], "consent_required"),
        (vec![("prompt", Some("login"))], "login_required"),
        (vec![("max_age", Some("soon"))], "invalid_request"),
        (vec![("nonce", Some(nonce.as_str()))], "invalid_request"),
    ] {
        let resp = harness.authorize(&request_with(&changes)).await;
        let url = location(&resp);
        assert!(
            url.as_str().starts_with(&format!("{REDIRECT_URI}?")),
            "{url}"
        );
        let params = query(&url);
        assert_eq!(param(&params, "error"), error, "{url}");
        assert_ne!(param(&params, "error_description"), "");
        assert_eq!(param(&params, "state"), STATE, "{url}");
        assert_eq!(param(&params, "iss"), OAUTH_ISSUER, "{url}");
        assert_eq!(params.len(), 4, "{url}");
    }

    let state = "s".repeat(1025);
    let resp = harness
        .authorize(&request_with(&[("state", Some(&state))]))
        .await;
    let url = location(&resp);
    let params = query(&url);
    assert_eq!(param(&params, "error"), "invalid_request");
    assert_eq!(param(&params, "iss"), OAUTH_ISSUER);
    assert!(params.iter().all(|(name, _)| name != "state"), "{url}");
    assert_eq!(harness.authorizations(), 0);

    let state = "s".repeat(1024);
    let resp = harness
        .authorize(&request_with(&[("state", Some(&state))]))
        .await;
    consent_request(&resp);
}

#[tokio::test]
async fn repeated_parameter_is_refused() {
    let harness = Harness::new().await;
    for (name, value) in [
        ("client_id", "https://evil.example/oauth/client.json"),
        ("redirect_uri", "https://evil.example/callback"),
        ("state", "other"),
        ("scope", "projects:read"),
        ("code_challenge_method", "plain"),
    ] {
        let mut repeated = request();
        repeated.push((name.to_owned(), value.to_owned()));
        let resp = harness.authorize(&repeated).await;
        assert_eq!(location(&resp), error_page("invalid_request"), "{name}");
    }
    assert_eq!(harness.authorizations(), 0);

    let mut unknown = request();
    unknown.push(("unknown".to_owned(), "1".to_owned()));
    unknown.push(("unknown".to_owned(), "2".to_owned()));
    consent_request(&harness.authorize(&unknown).await);
}

#[tokio::test]
async fn deny_needs_no_credential() {
    let harness = Harness::new().await;
    for bearer in [None, Some("garbage")] {
        let uuid = consent_request(&harness.authorize(&request()).await);
        let resp = harness.answer(uuid, false, bearer).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "cache-control"), "no-store");
        let url = redirect_url(resp).await;
        assert!(
            url.as_str().starts_with(&format!("{REDIRECT_URI}?")),
            "{url}"
        );
        let params = query(&url);
        assert_eq!(param(&params, "error"), "access_denied");
        assert_ne!(param(&params, "error_description"), "");
        assert_eq!(param(&params, "state"), STATE);
        assert_eq!(param(&params, "iss"), OAUTH_ISSUER);
        assert_eq!(params.len(), 4, "{url}");
        assert_eq!(harness.authorizations(), 0);
    }
}

#[tokio::test]
async fn request_is_answered_once() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Once", "once@example.com").await;

    let uuid = consent_request(&harness.authorize(&request()).await);
    let resp = harness.answer(uuid, true, Some(&user.token)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let code_hash = harness.authorization(uuid).code_hash;
    assert_eq!(
        harness.answer(uuid, true, Some(&user.token)).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        harness.answer(uuid, false, None).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(harness.view(uuid).await.status(), StatusCode::CONFLICT);
    assert_eq!(harness.authorization(uuid).code_hash, code_hash);

    let uuid = consent_request(&harness.authorize(&request()).await);
    assert_eq!(
        harness.answer(uuid, false, None).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        harness.answer(uuid, true, Some(&user.token)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        harness.answer(uuid, false, None).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn expired_request_is_gone() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Late", "late@example.com").await;

    let uuid = consent_request(&harness.authorize(&request()).await);
    harness.advance(30 * 60 - 1);
    assert_eq!(harness.view(uuid).await.status(), StatusCode::OK);
    harness.advance(1);
    assert_eq!(harness.view(uuid).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        harness.answer(uuid, true, Some(&user.token)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        harness.answer(uuid, false, None).await.status(),
        StatusCode::NOT_FOUND
    );
    assert!(!harness.authorization(uuid).is_answered());

    // A new request drops the expired ones.
    let fresh = consent_request(&harness.authorize(&request()).await);
    assert_eq!(harness.authorizations(), 1);

    // An approved request lasts only as long as its code.
    assert_eq!(
        harness
            .answer(fresh, true, Some(&user.token))
            .await
            .status(),
        StatusCode::OK
    );
    harness.advance(5 * 60 - 1);
    assert_eq!(harness.view(fresh).await.status(), StatusCode::CONFLICT);
    harness.advance(1);
    assert_eq!(harness.view(fresh).await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn approve_needs_a_user_credential() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Owner", "owner@example.com").await;
    let org = harness.server.create_org(&user, "Credentials").await;
    let project = harness
        .server
        .create_project(&user, &org, "Credentials")
        .await;
    let project_key = harness
        .server
        .create_project_key(&user, &project, "Run Key")
        .await;
    let uuid = consent_request(&harness.authorize(&request()).await);

    let resp = harness
        .answer(uuid, true, Some(project_key.key.as_ref()))
        .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Invalid body");
    assert!(body.contains("bencher_user_"), "{body}");
    assert!(!body.contains(project_key.key.as_ref()), "{body}");

    let malformed_key = "bencher_user_malformed";
    for (bearer, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("garbage"), StatusCode::UNAUTHORIZED),
        (Some(malformed_key), StatusCode::UNAUTHORIZED),
        (
            Some("bencher_oauth_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"),
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let resp = harness.answer(uuid, true, bearer).await;
        assert_eq!(resp.status(), status, "{bearer:?}");
        let body = resp.text().await.expect("Invalid body");
        assert!(!body.contains(malformed_key), "{body}");
        assert!(!body.contains("Malformed"), "{body}");
    }
    assert!(!harness.authorization(uuid).is_answered());

    let (_, key) = harness.create_user_key(&user).await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(&user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to lock the user");
    for (bearer, status) in [
        (user.token.as_str(), StatusCode::FORBIDDEN),
        (key.as_ref(), StatusCode::UNAUTHORIZED),
    ] {
        assert_eq!(
            harness.answer(uuid, true, Some(bearer)).await.status(),
            status
        );
    }
    assert!(!harness.authorization(uuid).is_answered());
}

// Kills an approval open to a deprecated API token, which only a session or a user key may give.
#[tokio::test]
async fn approve_refuses_an_api_token() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Legacy", "legacy@example.com").await;
    let token = seed_token(&harness.server, &user, "Legacy Token").token;
    let uuid = consent_request(&harness.authorize(&request()).await);

    let resp = harness.answer(uuid, true, Some(token.as_ref())).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Invalid body");
    assert!(!body.contains(token.as_ref()), "{body}");
    assert!(!harness.authorization(uuid).is_answered());
}

// A request approved with a pasted user key is bound to that key, so its grant dies with it.
#[tokio::test]
async fn approve_with_a_user_key_binds_the_key() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Keyed", "keyed@example.com").await;
    let other = harness.server.signup("Other", "other@example.com").await;
    let (key_uuid, key) = harness.create_user_key(&user).await;
    let (other_key_uuid, other_key) = harness.create_user_key(&other).await;

    let uuid = consent_request(&harness.authorize(&request()).await);
    let resp = harness.answer(uuid, true, Some(key.as_ref())).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let code: OAuthCode = param(&query(&redirect_url(resp).await), "code")
        .parse()
        .expect("Invalid code");
    let authorization = harness.authorization(uuid);
    assert_eq!(authorization.user_id, Some(harness.user_id(&user)));
    assert_eq!(
        authorization.user_key_id,
        Some(harness.user_key_id(key_uuid))
    );
    assert_eq!(authorization.code_hash, Some(OAuthCodeHash::from(&code)));

    // Another account's key approves as that account, never as this one.
    let uuid = consent_request(&harness.authorize(&request()).await);
    let resp = harness.answer(uuid, true, Some(other_key.as_ref())).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let authorization = harness.authorization(uuid);
    assert_eq!(authorization.user_id, Some(harness.user_id(&other)));
    assert_eq!(
        authorization.user_key_id,
        Some(harness.user_key_id(other_key_uuid))
    );
}

#[tokio::test]
async fn only_the_approver_key_is_bound() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Approver", "approver@example.com")
        .await;
    let other = harness.server.signup("Other", "other@example.com").await;
    let (other_key, _) = harness.create_user_key(&other).await;
    let uuid = consent_request(&harness.authorize(&request()).await);
    let code_hash = OAuthCodeHash::from(&OAuthCode::generate());
    let now = DateTime::try_from(NOW).expect("Invalid time");

    let answer = QueryOAuthAuthorization::approve(
        &mut harness.server.db_conn(),
        uuid,
        harness.user_id(&user),
        Some(harness.user_key_id(other_key)),
        &code_hash,
        now,
    )
    .expect("Failed to approve");
    assert!(matches!(answer, OAuthAnswer::ForeignKey), "{answer:?}");
    assert!(!harness.authorization(uuid).is_answered());

    let answer = QueryOAuthAuthorization::approve(
        &mut harness.server.db_conn(),
        uuid,
        harness.user_id(&other),
        Some(harness.user_key_id(other_key)),
        &code_hash,
        now,
    )
    .expect("Failed to approve");
    assert!(matches!(answer, OAuthAnswer::Recorded(_)), "{answer:?}");
}

// Pasted keys are guessable only as fast as sign in attempts allow; sessions are not charged.
#[tokio::test]
async fn pasted_key_is_charged_as_a_sign_in_attempt() {
    let harness = Harness::rate_limited(config(), None, one_attempt()).await;
    let user = harness
        .server
        .signup("Limited", "limited@example.com")
        .await;
    let (_, key) = harness.create_user_key(&user).await;
    let org = harness.server.create_org(&user, "Limited").await;
    let project = harness.server.create_project(&user, &org, "Limited").await;
    let project_key = harness
        .server
        .create_project_key(&user, &project, "Run Key")
        .await;

    let first = consent_request(&harness.authorize(&request()).await);
    let second = consent_request(&harness.authorize(&request()).await);
    assert_eq!(
        harness
            .answer(first, true, Some(key.as_ref()))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        harness
            .answer(second, true, Some(key.as_ref()))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        harness
            .answer(second, true, Some(project_key.key.as_ref()))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        harness
            .answer(second, true, Some(&user.token))
            .await
            .status(),
        StatusCode::OK
    );
    let third = consent_request(&harness.authorize(&request()).await);
    assert_eq!(
        harness
            .answer(third, true, Some(&user.token))
            .await
            .status(),
        StatusCode::OK
    );
}

// A metadata document fetch is charged as a sign in attempt, and a configured document is not.
#[tokio::test]
async fn client_fetch_is_charged_as_a_sign_in_attempt() {
    let mut config = config();
    config["clients"]
        .as_array_mut()
        .expect("Missing clients")
        .push(json!({"id": FETCHED_CLIENT_ID}));
    let harness =
        Harness::rate_limited(config, Some(ScriptedLookup::new(&[])), one_attempt()).await;

    consent_request(&harness.authorize(&request()).await);
    consent_request(&harness.authorize(&request()).await);
    let fetched = request_with(&[("client_id", Some(FETCHED_CLIENT_ID))]);
    assert_eq!(
        location(&harness.authorize(&fetched).await),
        error_page("invalid_client")
    );
    assert_eq!(
        location(&harness.authorize(&fetched).await),
        error_page("temporarily_unavailable")
    );
    consent_request(&harness.authorize(&request()).await);
}

// Why a metadata document could not be fetched never reaches the client, so the endpoint
// cannot tell anyone which names resolve to internal addresses.
#[tokio::test]
async fn fetch_failures_answer_alike() {
    let lookup = ScriptedLookup::new(&[("internal.example", "10.0.0.1")]);
    let harness = Harness::with_oauth(
        OAuthServer::with_egress(
            serde_json::from_value(json!({"issuer": OAUTH_ISSUER})).expect("Invalid config"),
            egress(Arc::clone(&lookup)),
        )
        .expect("Invalid OAuth server"),
        None,
    )
    .await;

    let mut answers = Vec::new();
    for client_id in [
        "https://internal.example/oauth/client.json",
        "https://missing.example/oauth/client.json",
    ] {
        let resp = harness
            .authorize(&request_with(&[("client_id", Some(client_id))]))
            .await;
        let status = resp.status();
        let headers = [
            "location",
            "cache-control",
            "access-control-allow-origin",
            "content-type",
            "content-length",
        ]
        .map(|name| {
            resp.headers()
                .get(name)
                .map(|value| value.as_bytes().to_vec())
        });
        let body = resp.bytes().await.expect("Invalid body");
        answers.push((status, headers, body));
    }
    assert_eq!(lookup.names(), vec!["internal.example", "missing.example"]);
    let [blocked, missing] = answers.as_slice() else {
        panic!("Expected two answers");
    };
    assert_eq!(blocked, missing);
    assert_eq!(blocked.0, StatusCode::FOUND);
    assert_eq!(
        blocked.1[0].as_deref(),
        Some(error_page("invalid_client").as_str().as_bytes())
    );
}

// The anonymous endpoints take the public request limit of the caller's address.
#[tokio::test]
async fn anonymous_requests_are_limited_per_address() {
    let harness = Harness::rate_limited(config(), None, json!({"requests": {"minute": 1}})).await;
    let uuid = consent_request(&harness.authorize(&request()).await);
    assert_eq!(
        location(&harness.authorize(&request()).await),
        error_page("temporarily_unavailable")
    );
    assert_eq!(
        harness.view(uuid).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        harness.answer(uuid, false, None).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(!harness.authorization(uuid).is_answered());
}

// A well-known host vouches for a client only when the answer also goes back to it.
#[tokio::test]
async fn verified_only_when_answered_on_the_same_host() {
    let json: JsonOAuthServer =
        serde_json::from_value(json!({ "issuer": OAUTH_ISSUER })).expect("Invalid OAuth config");
    let harness = Harness::with_oauth(
        OAuthServer::try_from(json).expect("Invalid OAuth server"),
        None,
    )
    .await;
    for (redirect_uri, verified) in [
        ("https://evil.example/cb", false),
        (
            "https://chatgpt.com/connector_platform_oauth_redirect",
            true,
        ),
    ] {
        let uuid = harness.pending("https://chatgpt.com/user/content.json", redirect_uri);
        let resp = harness.view(uuid).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let authorization: JsonOAuthAuthorization =
            resp.json().await.expect("Invalid authorization");
        assert_eq!(authorization.client.host, "chatgpt.com");
        assert_eq!(authorization.client.verified, verified, "{redirect_uri}");
    }
}

#[tokio::test]
async fn protocol_responses_allow_any_origin() {
    let harness = Harness::new().await;
    for resp in [
        harness.authorize(&request()).await,
        harness
            .authorize(&request_with(&[("client_id", None)]))
            .await,
        harness
            .authorize(&request_with(&[("response_type", Some("token"))]))
            .await,
    ] {
        assert_eq!(resp.status(), StatusCode::FOUND);
        assert_eq!(header(&resp, "access-control-allow-origin"), "*");
        assert_eq!(header(&resp, "cache-control"), "no-store");
    }
    for (path, methods) in [
        ("/oauth/authorize".to_owned(), "GET"),
        (
            format!("/v0/oauth/authorizations/{}", OAuthAuthorizationUuid::new()),
            "GET, POST",
        ),
        ("/oauth/token".to_owned(), "POST"),
        ("/oauth/revoke".to_owned(), "POST"),
        ("/.well-known/oauth-protected-resource".to_owned(), "GET"),
        (
            "/.well-known/oauth-protected-resource/mcp".to_owned(),
            "GET",
        ),
        ("/.well-known/oauth-authorization-server".to_owned(), "GET"),
    ] {
        let resp = harness
            .client
            .request(Method::OPTIONS, harness.server.api_url(&path))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&resp, "access-control-allow-origin"), "*");
        assert_eq!(header(&resp, "access-control-allow-methods"), methods);
    }
}

// Every new route is absent until OAuth is configured.
#[tokio::test]
async fn oauth_unset_is_not_found() {
    let server = TestServer::new().await;
    let authorization = format!("/v0/oauth/authorizations/{}", OAuthAuthorizationUuid::new());
    let authorize = format!(
        "/oauth/authorize?{}",
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(request())
            .finish()
    );
    for (method, path) in [
        (Method::GET, authorize.as_str()),
        (Method::OPTIONS, "/oauth/authorize"),
        (Method::GET, authorization.as_str()),
        (Method::POST, authorization.as_str()),
        (Method::OPTIONS, authorization.as_str()),
        (Method::POST, "/oauth/token"),
        (Method::OPTIONS, "/oauth/token"),
        (Method::POST, "/oauth/revoke"),
        (Method::OPTIONS, "/oauth/revoke"),
        (Method::GET, "/.well-known/oauth-protected-resource"),
        (Method::OPTIONS, "/.well-known/oauth-protected-resource"),
        (Method::GET, "/.well-known/oauth-protected-resource/mcp"),
        (Method::OPTIONS, "/.well-known/oauth-protected-resource/mcp"),
        (Method::GET, "/.well-known/oauth-authorization-server"),
        (Method::OPTIONS, "/.well-known/oauth-authorization-server"),
    ] {
        let resp = server
            .client
            .request(method.clone(), server.api_url(path))
            .json(&json!({"approve": false}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} {path}");
    }
}

// The whole flow through the endpoints the server advertises: an app is approved, exchanges its
// code, calls a tool with the access token, refreshes, and is revoked.
#[tokio::test]
async fn advertised_flow_calls_a_tool() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Flow", "flow@example.com").await;
    let org = harness.server.create_org(&user, "Flow").await;
    let project = harness.server.create_project(&user, &org, "Flow").await;
    let slug: &str = project.slug.as_ref();

    let resp = harness
        .discover("/.well-known/oauth-authorization-server", "api.example.com")
        .await;
    let metadata: Value = resp.json().await.expect("Invalid metadata");
    let authorize = advertised_path(&metadata, "authorization_endpoint");
    let token = advertised_path(&metadata, "token_endpoint");
    let revoke = advertised_path(&metadata, "revocation_endpoint");

    let resp = harness
        .with_address(
            harness
                .client
                .get(harness.server.api_url(&authorize))
                .query(&request()),
        )
        .send()
        .await
        .expect("Request failed");
    let uuid = consent_request(&resp);
    let url = redirect_url(harness.answer(uuid, true, Some(&user.token)).await).await;
    let code = param(&query(&url), "code").to_owned();

    let tokens = token_body(harness.post_form(&token, &exchange(&code), None).await).await;
    assert_eq!(tokens["token_type"], "Bearer");
    assert_eq!(tokens["expires_in"], 60 * 60);
    assert_eq!(tokens["scope"], "projects:read projects:write");
    let access_token = string(&tokens, "access_token");
    let refresh_token = string(&tokens, "refresh_token");
    assert!(access_token.starts_with("bencher_oauth_"), "{access_token}");
    assert!(
        project_slugs(harness.list_projects(&org.slug, &access_token).await)
            .await
            .contains(&slug.to_owned())
    );

    let tokens = token_body(
        harness
            .post_form(&token, &refresh(&refresh_token), None)
            .await,
    )
    .await;
    let access_token = string(&tokens, "access_token");
    let refresh_token = string(&tokens, "refresh_token");
    assert!(
        project_slugs(harness.list_projects(&org.slug, &access_token).await)
            .await
            .contains(&slug.to_owned())
    );

    let resp = harness
        .post_form(
            &revoke,
            &params(&[("token", refresh_token.as_str()), ("client_id", CLIENT_ID)]),
            None,
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// A code goes only to the client holding the verifier behind its challenge.
#[tokio::test]
async fn code_needs_its_verifier() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Verifier", "verifier@example.com")
        .await;
    let code = harness.code(&request(), &user.token).await;

    let resp = harness
        .token(&exchange_with(
            &code,
            &[("code_verifier", Some(OTHER_VERIFIER))],
        ))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    token_body(harness.token(&exchange(&code)).await).await;
}

// A code exchanged twice was stolen, so the grant made from it is revoked.
#[tokio::test]
async fn reused_code_revokes_its_grant() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Reuse", "reuse@example.com").await;
    let code = harness.code(&request(), &user.token).await;
    let tokens = token_body(harness.token(&exchange(&code)).await).await;
    let access_token = string(&tokens, "access_token");
    let refresh_token = string(&tokens, "refresh_token");
    project_slugs(harness.list_projects(user.uuid, &access_token).await).await;

    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// A code is exchanged only by its client, for the redirect URI and resource it was asked for,
// before it expires. A refused exchange leaves the code as it was.
#[tokio::test]
async fn code_is_bound_to_its_request() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Bound", "bound@example.com").await;
    let code = harness.code(&request(), &user.token).await;
    for changes in [
        [("client_id", Some(LOOPBACK_CLIENT_ID))],
        [(
            "redirect_uri",
            Some("https://client.example/callback?tenant=1"),
        )],
        [("resource", Some("https://mcp.example.com/mcp"))],
    ] {
        let resp = harness.token(&exchange_with(&code, &changes)).await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    }
    // The redirect URI need not be sent again, and the resource may be spelled any way it normalizes.
    token_body(
        harness
            .token(&exchange_with(
                &code,
                &[
                    ("redirect_uri", None),
                    ("resource", Some("https://API.example.com/mcp/")),
                ],
            ))
            .await,
    )
    .await;

    let code = harness.code(&request(), &user.token).await;
    harness.advance(5 * 60);
    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// A loopback redirect URI is compared as the authorization request sent it, port and all.
#[tokio::test]
async fn loopback_redirect_uri_is_compared_as_sent() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Loopback", "loopback@example.com")
        .await;
    let redirect_uri = "http://localhost:53682/callback";
    let code = harness
        .code(&loopback_request(redirect_uri), &user.token)
        .await;
    for other in [
        "http://localhost:53683/callback",
        "http://127.0.0.1:53682/callback",
        "http://localhost/callback",
    ] {
        let resp = harness.token(&loopback_exchange(&code, other)).await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    }
    token_body(harness.token(&loopback_exchange(&code, redirect_uri)).await).await;
}

// A client that did not register the refresh_token grant type gets no refresh token.
#[tokio::test]
async fn refresh_token_only_when_registered() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Once", "once@example.com").await;
    let redirect_uri = "http://127.0.0.1:5/callback";
    let code = harness
        .code(&loopback_request(redirect_uri), &user.token)
        .await;
    let tokens = token_body(harness.token(&loopback_exchange(&code, redirect_uri)).await).await;
    assert!(tokens.get("refresh_token").is_none(), "{tokens}");
    project_slugs(
        harness
            .list_projects(user.uuid, &string(&tokens, "access_token"))
            .await,
    )
    .await;
}

// A token request may name only a resource this server serves, and only the grant's own.
#[tokio::test]
async fn token_resource_must_be_served() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Resource", "resource@example.com")
        .await;
    let code = harness.code(&request(), &user.token).await;
    for resource in ["https://evil.example/mcp", "not a url"] {
        let resp = harness
            .token(&exchange_with(&code, &[("resource", Some(resource))]))
            .await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_target").await;
    }
    let tokens = token_body(harness.token(&exchange(&code)).await).await;
    let refresh_token = string(&tokens, "refresh_token");
    for (resource, error) in [
        ("https://evil.example/mcp", "invalid_target"),
        ("https://mcp.example.com/mcp", "invalid_grant"),
    ] {
        let resp = harness
            .token(&refresh_with(
                &refresh_token,
                &[("resource", Some(resource))],
            ))
            .await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, error).await;
    }
    let tokens = token_body(
        harness
            .token(&refresh_with(
                &refresh_token,
                &[("resource", Some("https://api.example.com/mcp"))],
            ))
            .await,
    )
    .await;

    // A resource the server no longer serves ends the grant's refreshes, and a code's exchange.
    let unserved = "https://retired.example.com/mcp";
    diesel::update(schema::oauth_grant::table)
        .set(schema::oauth_grant::resource.eq(unserved))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to retire the resource");
    let resp = harness
        .token(&refresh(&string(&tokens, "refresh_token")))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    let code = harness.code(&request(), &user.token).await;
    diesel::update(schema::oauth_authorization::table)
        .set(schema::oauth_authorization::resource.eq(unserved))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to retire the resource");
    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// Each refresh token is good for one refresh. One presented again was stolen, so its grant is revoked.
#[tokio::test]
async fn reused_refresh_token_revokes_its_grant() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Rotate", "rotate@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;

    let tokens = token_body(harness.token(&refresh(&refresh_token)).await).await;
    assert_eq!(tokens["expires_in"], 60 * 60);
    assert_eq!(tokens["scope"], "projects:read projects:write");
    let access_token = string(&tokens, "access_token");
    let rotated = string(&tokens, "refresh_token");
    assert_ne!(rotated, refresh_token);
    project_slugs(harness.list_projects(user.uuid, &access_token).await).await;

    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let resp = harness.token(&refresh(&rotated)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

#[tokio::test]
async fn refresh_token_is_bound_to_its_client() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Client", "client@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;
    let resp = harness
        .token(&refresh_with(
            &refresh_token,
            &[("client_id", Some(LOOPBACK_CLIENT_ID))],
        ))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    token_body(harness.token(&refresh(&refresh_token)).await).await;
}

// An access token is never a refresh token, and presenting one as such changes nothing.
#[tokio::test]
async fn access_token_does_not_refresh() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Kind", "kind@example.com").await;
    let (access_token, refresh_token) = harness.tokens(&user).await;
    let resp = harness.token(&refresh(&access_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    project_slugs(harness.list_projects(user.uuid, &access_token).await).await;
    token_body(harness.token(&refresh(&refresh_token)).await).await;
}

// A grant lasts a year from approval. Each refresh gives its new refresh token 30 more days, never
// past the grant's end.
#[tokio::test]
async fn grant_lasts_a_year() {
    const DAY: i64 = 24 * 60 * 60;
    let harness = Harness::new().await;
    let user = harness.server.signup("Year", "year@example.com").await;
    let (_, mut refresh_token) = harness.tokens(&user).await;
    assert_eq!(harness.grant().expiration.timestamp(), NOW + 365 * DAY);

    for _ in 0..12 {
        harness.advance(29 * DAY);
        let tokens = token_body(harness.token(&refresh(&refresh_token)).await).await;
        refresh_token = string(&tokens, "refresh_token");
    }
    let newest = harness.token_rows().pop().expect("Missing token");
    assert_eq!(newest.kind, OAuthTokenKind::Refresh);
    assert_eq!(newest.expiration.timestamp(), NOW + 365 * DAY);

    harness.advance((365 - 12 * 29) * DAY);
    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// A code is not exchanged once its approver is locked or the key they approved with is dead,
// and no grant is written.
#[tokio::test]
async fn exchange_needs_a_live_approver() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Lapsed", "lapsed@example.com").await;
    let (_, key) = harness.create_user_key_with_ttl(&user, Some(60)).await;
    let code = harness.code(&request(), key.as_ref()).await;
    harness.advance(2 * 60);
    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(harness.grants(), 0);

    let other = harness.server.signup("Locked", "locked@example.com").await;
    let code = harness.code(&request(), &other.token).await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(&other.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to lock the user");
    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(harness.grants(), 0);
}

// A grant approved with a pasted key lasts no longer than the key, and dies with it.
#[tokio::test]
async fn keyed_grant_dies_with_its_key() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Keyed", "keyed@example.com").await;
    let ttl = 30 * 60;
    let (key_uuid, key) = harness.create_user_key_with_ttl(&user, Some(ttl)).await;
    let code = harness.code(&request(), key.as_ref()).await;

    let tokens = token_body(harness.token(&exchange(&code)).await).await;
    assert_eq!(tokens["expires_in"], ttl);
    let grant = harness.grant();
    assert_eq!(grant.user_key_id, Some(harness.user_key_id(key_uuid)));
    assert_eq!(grant.expiration.timestamp(), NOW + i64::from(ttl));
    assert!(
        harness
            .token_rows()
            .iter()
            .all(|token| token.expiration.timestamp() == NOW + i64::from(ttl))
    );

    let tokens = token_body(
        harness
            .token(&refresh(&string(&tokens, "refresh_token")))
            .await,
    )
    .await;
    harness.revoke_user_key(&user, key_uuid).await;
    let resp = harness
        .token(&refresh(&string(&tokens, "refresh_token")))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

#[tokio::test]
async fn locked_user_cannot_refresh() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Locked", "locked@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(&user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to lock the user");
    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// A refresh token lasts 30 days. Each refresh marks the grant used and drops its expired tokens,
// keeping used refresh tokens until they expire, so reuse is still caught.
#[tokio::test]
async fn refresh_token_lasts_thirty_days() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Month", "month@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;

    harness.advance(2 * 60 * 60);
    let tokens = token_body(harness.token(&refresh(&refresh_token)).await).await;
    let kinds = harness
        .token_rows()
        .iter()
        .map(|token| (token.kind, token.used.is_some()))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            (OAuthTokenKind::Refresh, true),
            (OAuthTokenKind::Access, false),
            (OAuthTokenKind::Refresh, false),
        ]
    );
    assert_eq!(harness.grant().modified.timestamp(), NOW + 2 * 60 * 60);

    harness.advance(30 * 24 * 60 * 60);
    let resp = harness
        .token(&refresh(&string(&tokens, "refresh_token")))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// A refresh may ask for no more than the grant allows. Unknown scopes are ignored, and the tokens
// always carry the grant's scopes.
#[tokio::test]
async fn refresh_scope_is_within_the_grant() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Scope", "scope@example.com").await;
    for (granted, asked, refused) in [
        (
            "projects:read",
            "projects:read offline_access",
            Some("projects:read projects:write"),
        ),
        ("projects:write", "projects:read", None),
    ] {
        let code = harness
            .code(&request_with(&[("scope", Some(granted))]), &user.token)
            .await;
        let tokens = token_body(harness.token(&exchange(&code)).await).await;
        assert_eq!(tokens["scope"], granted);
        let refresh_token = string(&tokens, "refresh_token");
        if let Some(refused) = refused {
            let resp = harness
                .token(&refresh_with(&refresh_token, &[("scope", Some(refused))]))
                .await;
            assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_scope").await;
        }
        let tokens = token_body(
            harness
                .token(&refresh_with(&refresh_token, &[("scope", Some(asked))]))
                .await,
        )
        .await;
        assert_eq!(tokens["scope"], granted, "{asked}");
    }
}

// Only public clients are served, so any attempt to authenticate a client is refused.
#[tokio::test]
async fn client_credentials_are_refused() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Credentials", "credentials@example.com")
        .await;
    let code = harness.code(&request(), &user.token).await;
    for (changes, authorization) in [
        (
            vec![("client_assertion", Some("eyJhbGciOiJSUzI1NiJ9.e30.c2ln"))],
            None,
        ),
        (
            vec![(
                "client_assertion_type",
                Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer"),
            )],
            None,
        ),
        (vec![("client_secret", Some("secret"))], None),
        (vec![], Some("Basic Y2xpZW50OnNlY3JldA==")),
        (vec![], Some("Basic")),
    ] {
        let resp = harness
            .post_form(
                "/oauth/token",
                &exchange_with(&code, &changes),
                authorization,
            )
            .await;
        assert_eq!(
            header(&resp, "www-authenticate"),
            format!(r#"Basic realm="{OAUTH_ISSUER}""#)
        );
        assert_oauth_error(resp, StatusCode::UNAUTHORIZED, "invalid_client").await;
    }
    let resp = harness
        .revoke(&params(&[
            ("token", code.as_str()),
            ("client_id", CLIENT_ID),
            ("client_secret", "secret"),
        ]))
        .await;
    assert_oauth_error(resp, StatusCode::UNAUTHORIZED, "invalid_client").await;

    // A bearer credential is not client authentication.
    token_body(
        harness
            .post_form("/oauth/token", &exchange(&code), Some("Bearer unrelated"))
            .await,
    )
    .await;
}

// Every refusal is an OAuth error that is never cached and never quotes what was sent.
#[tokio::test]
async fn token_errors_are_oauth_errors() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Errors", "errors@example.com").await;
    let code = harness.code(&request(), &user.token).await;

    let mut repeated = exchange(&code);
    repeated.push(("code_verifier".to_owned(), OTHER_VERIFIER.to_owned()));
    for (params, error) in [
        (repeated, "invalid_request"),
        (
            exchange_with(&code, &[("grant_type", None)]),
            "invalid_request",
        ),
        (
            exchange_with(&code, &[("grant_type", Some("client_credentials"))]),
            "unsupported_grant_type",
        ),
        (
            exchange_with(&code, &[("client_id", None)]),
            "invalid_request",
        ),
        (
            exchange_with(&code, &[("code_verifier", None)]),
            "invalid_request",
        ),
        (
            exchange_with(&code, &[("code_verifier", Some(SHORT_CHALLENGE))]),
            "invalid_request",
        ),
        (
            exchange_with(&code, &[("client_id", Some(LOOPBACK_CLIENT_ID))]),
            "invalid_grant",
        ),
        (
            exchange_with(&code, &[("code", Some("bencher_code_short"))]),
            "invalid_grant",
        ),
        (
            refresh_with(&code, &[("client_id", None)]),
            "invalid_request",
        ),
    ] {
        let body =
            assert_oauth_error(harness.token(&params).await, StatusCode::BAD_REQUEST, error).await;
        assert!(!body.contains(&code), "{body}");
    }

    let mut unknown = exchange(&code);
    unknown.push(("unknown".to_owned(), "1".to_owned()));
    unknown.push(("unknown".to_owned(), "2".to_owned()));
    token_body(harness.token(&unknown).await).await;
}

// A refresh token revokes its grant, and an access token is deleted. Any other token changes
// nothing, and every answer is the same.
#[tokio::test]
async fn revoke_tokens() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Revoke", "revoke@example.com").await;
    let (access_token, refresh_token) = harness.tokens(&user).await;

    let resp = harness
        .revoke(&params(&[
            ("token", access_token.as_str()),
            ("client_id", CLIENT_ID),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    assert_eq!(header(&resp, "access-control-allow-origin"), "*");
    assert_eq!(resp.text().await.expect("Invalid body"), "");
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let tokens = token_body(harness.token(&refresh(&refresh_token)).await).await;
    let access_token = string(&tokens, "access_token");
    let refresh_token = string(&tokens, "refresh_token");

    for (token, client_id) in [
        (refresh_token.as_str(), LOOPBACK_CLIENT_ID),
        (access_token.as_str(), LOOPBACK_CLIENT_ID),
        ("bencher_refresh_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh", CLIENT_ID),
        ("garbage", CLIENT_ID),
    ] {
        let resp = harness
            .revoke(&params(&[("token", token), ("client_id", client_id)]))
            .await;
        assert_eq!(resp.status(), StatusCode::OK, "{token}");
    }
    project_slugs(harness.list_projects(user.uuid, &access_token).await).await;

    let resp = harness
        .revoke(&params(&[
            ("token", refresh_token.as_str()),
            ("token_type_hint", "access_token"),
            ("client_id", CLIENT_ID),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;

    let resp = harness.revoke(&params(&[("client_id", CLIENT_ID)])).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_request").await;
}

// A token or revocation request for a live grant is charged to its user, and one refused for that
// rate changes nothing: the code can still be exchanged, and the grant and its refresh token live on.
#[tokio::test]
async fn token_requests_are_charged_to_the_user() {
    let harness = Harness::with_rate_limiting(json!({"user": {"requests": {"minute": 2}}})).await;
    let user = harness.server.signup("Busy", "busy@example.com").await;
    let code = harness.code(&request(), &user.token).await;
    let tokens = token_body(harness.token(&exchange(&code)).await).await;
    let refresh_token = string(&tokens, "refresh_token");
    let resp = harness.token(&refresh(&refresh_token)).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
    let resp = harness
        .revoke(&params(&[
            ("token", refresh_token.as_str()),
            ("client_id", CLIENT_ID),
        ]))
        .await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
    assert!(
        harness
            .token_rows()
            .iter()
            .all(|token| token.used.is_none())
    );
    assert!(harness.grant().revoked.is_none());

    let other = harness.server.signup("Other", "other@example.com").await;
    let code = harness.code(&request(), &other.token).await;
    harness.code(&request(), &other.token).await;
    let resp = harness.token(&exchange(&code)).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
    assert_eq!(harness.grants(), 1);
}

// Only a request that names no known code or token is anonymous, so only it is charged to its
// address. Any other request gets its real answer however spent the address is, since an app's
// users share its addresses.
#[tokio::test]
async fn only_unknown_credentials_are_charged_per_address() {
    let harness = Harness::rate_limited(config(), None, json!({"requests": {"minute": 4}})).await;
    let user = harness
        .server
        .signup("Address", "address@example.com")
        .await;
    let (_, live) = harness.tokens(&user).await;
    let (_, expiring) = harness.tokens(&user).await;
    let (revoked_access, revoked_refresh) = harness.tokens(&user).await;

    // A valid revocation is not charged, so the address still answers one unknown code.
    let resp = harness
        .revoke(&params(&[
            ("token", revoked_access.as_str()),
            ("client_id", CLIENT_ID),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let unknown = OAuthCode::generate();
    let resp = harness.token(&exchange(unknown.as_ref())).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    let resp = harness.token(&exchange(unknown.as_ref())).await;
    assert_eq!(header(&resp, "retry-after"), "60");
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;

    // Behind the spent address, a used refresh token still revokes its live grant, and a dead
    // one is still told so.
    let tokens = token_body(harness.token(&refresh(&live)).await).await;
    let access_token = string(&tokens, "access_token");
    let resp = harness.token(&refresh(&live)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let resp = harness
        .token(&refresh(&string(&tokens, "refresh_token")))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    harness.expire_token(&expiring);
    let resp = harness.token(&refresh(&expiring)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    let resp = harness
        .revoke(&params(&[
            ("token", revoked_refresh.as_str()),
            ("client_id", CLIENT_ID),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = harness.token(&refresh(&revoked_refresh)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;

    let resp = harness
        .revoke(&params(&[("token", "garbage"), ("client_id", CLIENT_ID)]))
        .await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
    let resp = harness.token(&exchange(unknown.as_ref())).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
}

// Kills a refusal of a known credential charged to its user, or never throttled: a revoked app
// flooding its dead refresh token is throttled on that token's grant alone, at both endpoints, and
// its user is spared.
#[tokio::test]
async fn dead_credential_floods_are_throttled_per_grant() {
    let harness = Harness::with_rate_limiting(json!({
        "user": {"requests": {"minute": 4}},
        "public": {"requests": {"minute": 3}},
    }))
    .await;
    let user = harness
        .server
        .signup("Flooded", "flooded@example.com")
        .await;
    let (_, refresh_token) = harness.tokens(&user).await;
    let revoke = params(&[("token", refresh_token.as_str()), ("client_id", CLIENT_ID)]);
    assert_eq!(harness.revoke(&revoke).await.status(), StatusCode::OK);

    for _ in 0..2 {
        let resp = harness.token(&refresh(&refresh_token)).await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    }
    assert_eq!(harness.revoke(&revoke).await.status(), StatusCode::OK);
    for resp in [
        harness.token(&refresh(&refresh_token)).await,
        harness.revoke(&revoke).await,
    ] {
        assert_eq!(header(&resp, "retry-after"), "60");
        assert_oauth_error(
            resp,
            StatusCode::TOO_MANY_REQUESTS,
            "temporarily_unavailable",
        )
        .await;
    }
    assert_eq!(harness.user_status(&user).await, StatusCode::OK);
}

// Kills a code refusal charged to its user, never throttled, or throttling the valid exchange:
// guesses at a code's verifier are throttled on that code alone, and its client still exchanges it.
#[tokio::test]
async fn wrong_verifiers_are_throttled_per_code() {
    let harness = Harness::with_rate_limiting(json!({
        "user": {"requests": {"minute": 4}},
        "public": {"requests": {"minute": 2}},
    }))
    .await;
    let user = harness
        .server
        .signup("Guessed", "guessed@example.com")
        .await;
    let code = harness.code(&request(), &user.token).await;
    let guess = exchange_with(&code, &[("code_verifier", Some(OTHER_VERIFIER))]);
    for _ in 0..2 {
        let resp = harness.token(&guess).await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    }
    let resp = harness.token(&guess).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;

    token_body(harness.token(&exchange(&code)).await).await;
    assert_eq!(harness.user_status(&user).await, StatusCode::OK);
}

// Kills a limit that blocks reuse, or a valid refresh left uncharged: a replayed refresh token
// revokes its live grant even once its user and its grant's refusals are spent.
#[tokio::test]
async fn reuse_revokes_past_every_limit() {
    let harness = Harness::with_rate_limiting(json!({
        "user": {"requests": {"minute": 3}},
        "public": {"requests": {"minute": 2}},
    }))
    .await;
    let user = harness.server.signup("Replay", "replay@example.com").await;
    let (_, replayed) = harness.tokens(&user).await;
    let tokens = token_body(harness.token(&refresh(&replayed)).await).await;
    let live = string(&tokens, "refresh_token");
    let resp = harness.token(&refresh(&live)).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;
    let as_other_client = refresh_with(&live, &[("client_id", Some(LOOPBACK_CLIENT_ID))]);
    for _ in 0..2 {
        let resp = harness.token(&as_other_client).await;
        assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    }
    let resp = harness.token(&as_other_client).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;

    let resp = harness.token(&refresh(&replayed)).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert!(harness.grant().revoked.is_some());
}

// Kills a client taken off the allowlist whose codes or grants still work.
#[tokio::test]
async fn unlisted_client_is_disconnected() {
    let harness = Harness::new().await;
    let user = harness
        .server
        .signup("Unlisted", "unlisted@example.com")
        .await;
    let (access_token, refresh_token) = harness.tokens(&user).await;
    let code = harness.code(&request(), &user.token).await;
    let unlisted = "https://unlisted.example/oauth/client.json";
    diesel::update(schema::oauth_grant::table)
        .set(schema::oauth_grant::client_id.eq(unlisted))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to move the grant");
    diesel::update(schema::oauth_authorization::table)
        .set(schema::oauth_authorization::client_id.eq(unlisted))
        .execute(&mut harness.server.db_conn())
        .expect("Failed to move the code");

    let resp = harness
        .token(&exchange_with(&code, &[("client_id", Some(unlisted))]))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    let resp = harness
        .token(&refresh_with(
            &refresh_token,
            &[("client_id", Some(unlisted))],
        ))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert_eq!(
        harness
            .list_projects(user.uuid, &access_token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// A code or token gone between the read path and the writer is refused for what it was, never
// charged to the address, which is spent here, so a charge would answer `429`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn vanished_credentials_are_not_charged_per_address() {
    let harness = Harness::rate_limited(config(), None, json!({"requests": {"minute": 4}})).await;
    let user = harness.server.signup("Gone", "gone@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;
    let (access_token, _) = harness.tokens(&user).await;
    let code = harness.code(&request(), &user.token).await;
    let unknown = OAuthCode::generate();
    let resp = harness.token(&exchange(unknown.as_ref())).await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    let resp = harness.token(&exchange(unknown.as_ref())).await;
    assert_oauth_error(
        resp,
        StatusCode::TOO_MANY_REQUESTS,
        "temporarily_unavailable",
    )
    .await;

    let refresh_hash = OAuthTokenHash::from(
        &refresh_token
            .parse::<OAuthRefreshToken>()
            .expect("Invalid token"),
    );
    let resp = harness
        .post_racing_the_writer("/oauth/token", &refresh(&refresh_token), |conn| {
            delete_token(conn, &refresh_hash);
        })
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;

    let code_hash = OAuthCodeHash::from(&code.parse::<OAuthCode>().expect("Invalid code"));
    let resp = harness
        .post_racing_the_writer("/oauth/token", &exchange(&code), |conn| {
            diesel::delete(
                schema::oauth_authorization::table
                    .filter(schema::oauth_authorization::code_hash.eq(&code_hash)),
            )
            .execute(conn)
            .expect("Failed to delete the request");
        })
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;

    let access_hash = OAuthTokenHash::from(
        &access_token
            .parse::<OAuthAccessToken>()
            .expect("Invalid token"),
    );
    let resp = harness
        .post_racing_the_writer(
            "/oauth/revoke",
            &params(&[("token", access_token.as_str()), ("client_id", CLIENT_ID)]),
            |conn| delete_token(conn, &access_hash),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// The writer judges a request again as the database then stands: a refresh that passed the read
// path is refused once its grant is revoked, and issues nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_writer_judges_again() {
    let harness = Harness::new().await;
    let user = harness.server.signup("Again", "again@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;
    let resp = harness
        .post_racing_the_writer("/oauth/token", &refresh(&refresh_token), |conn| {
            diesel::update(schema::oauth_grant::table)
                .set(
                    schema::oauth_grant::revoked.eq(DateTime::try_from(NOW).expect("Invalid time")),
                )
                .execute(conn)
                .expect("Failed to revoke the grant");
        })
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
    assert!(
        harness
            .token_rows()
            .iter()
            .all(|token| token.used.is_none())
    );
    assert_eq!(harness.token_rows().len(), 2);
}

// With another connection holding the database's write lock, every request that must not write
// still gets its real answer: one that took the writer would wait out the busy timeout and fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn refusals_never_take_the_writer() {
    let harness = Harness::with_rate_limiting(json!({"user": {"requests": {"minute": 6}}})).await;
    let as_other_client =
        |token: &str| refresh_with(token, &[("client_id", Some(LOOPBACK_CLIENT_ID))]);
    let revoke =
        |token: &str, client_id: &str| params(&[("token", token), ("client_id", client_id)]);
    let user = harness
        .server
        .signup("Expired", "expired@example.com")
        .await;
    let (_, expired) = harness.tokens(&user).await;
    harness.expire_token(&expired);
    let user = harness
        .server
        .signup("Revoked", "revoked@example.com")
        .await;
    let (_, replayed) = harness.tokens(&user).await;
    let tokens = token_body(harness.token(&refresh(&replayed)).await).await;
    let resp = harness
        .revoke(&revoke(&string(&tokens, "refresh_token"), CLIENT_ID))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let user = harness.server.signup("Live", "live@example.com").await;
    let (live_access, live_refresh) = harness.tokens(&user).await;
    let code = harness.code(&request(), &user.token).await;
    let user = harness.server.signup("Busy", "busy@example.com").await;
    let (_, mut throttled) = harness.tokens(&user).await;
    for _ in 0..4 {
        let tokens = token_body(harness.token(&refresh(&throttled)).await).await;
        throttled = string(&tokens, "refresh_token");
    }

    let unknown_code = OAuthCode::generate();
    let unknown_refresh = OAuthRefreshToken::generate();
    let wrong_verifier = exchange_with(&code, &[("code_verifier", Some(OTHER_VERIFIER))]);
    let (token, revocation) = ("/oauth/token", "/oauth/revoke");
    let (refused, revoked) = (StatusCode::BAD_REQUEST, StatusCode::OK);
    let cases = [
        (token, exchange(unknown_code.as_ref()), refused),
        (token, wrong_verifier, refused),
        (token, refresh(unknown_refresh.as_ref()), refused),
        (token, refresh(&expired), refused),
        (token, refresh(&replayed), refused),
        (token, as_other_client(&live_refresh), refused),
        (token, refresh(&live_access), refused),
        (token, refresh(&throttled), StatusCode::TOO_MANY_REQUESTS),
        (revocation, revoke("garbage", CLIENT_ID), revoked),
        (
            revocation,
            revoke(unknown_refresh.as_ref(), CLIENT_ID),
            revoked,
        ),
        (revocation, revoke(&replayed, CLIENT_ID), revoked),
        (
            revocation,
            revoke(&live_refresh, LOOPBACK_CLIENT_ID),
            revoked,
        ),
    ];

    let mut lock = harness.server.db_conn();
    lock.batch_execute("BEGIN IMMEDIATE")
        .expect("Failed to take the write lock");
    let mut answers = Vec::new();
    for (path, params, _) in &cases {
        answers.push(harness.post_form(path, params, None).await.status());
    }
    lock.batch_execute("ROLLBACK")
        .expect("Failed to release the write lock");
    assert_eq!(answers, cases.map(|(_, _, status)| status));
}

// A user over their own limit does not spend the limit of the address they share with others.
#[tokio::test]
async fn throttled_user_does_not_spend_the_address() {
    let harness = Harness::with_rate_limiting(json!({
        "user": {"requests": {"minute": 2}},
        "public": {"requests": {"minute": 2}},
    }))
    .await;
    let user = harness.server.signup("Heavy", "heavy@example.com").await;
    let (_, refresh_token) = harness.tokens(&user).await;
    for _ in 0..2 {
        let resp = harness.token(&refresh(&refresh_token)).await;
        assert_oauth_error(
            resp,
            StatusCode::TOO_MANY_REQUESTS,
            "temporarily_unavailable",
        )
        .await;
    }
    let resp = harness
        .token(&exchange(OAuthCode::generate().as_ref()))
        .await;
    assert_oauth_error(resp, StatusCode::BAD_REQUEST, "invalid_grant").await;
}

// The protected resource metadata names the MCP server the request reached by its `Host`, or the
// canonical one for any other `Host`, which it never repeats.
#[tokio::test]
async fn protected_resource_follows_the_host() {
    let harness = Harness::new().await;
    for path in [
        "/.well-known/oauth-protected-resource/mcp",
        "/.well-known/oauth-protected-resource",
    ] {
        for (host, resource) in [
            ("mcp.example.com", "https://mcp.example.com/mcp"),
            ("MCP.example.com:443", "https://mcp.example.com/mcp"),
            ("api.example.com", "https://api.example.com/mcp"),
            ("evil.example", "https://api.example.com/mcp"),
            ("mcp.example.com:8443", "https://api.example.com/mcp"),
        ] {
            let resp = harness.discover(path, host).await;
            assert_metadata_headers(&resp);
            let body = resp.text().await.expect("Invalid body");
            assert!(!body.contains("evil") && !body.contains("8443"), "{body}");
            let metadata: Value = serde_json::from_str(&body).expect("Invalid metadata");
            assert_eq!(
                metadata,
                json!({
                    "resource": resource,
                    "authorization_servers": [OAUTH_ISSUER],
                    "scopes_supported": ["projects:read", "projects:write"],
                    "bearer_methods_supported": ["header"],
                    "resource_name": "Bencher",
                    "resource_documentation": "https://bencher.dev/docs/how-to/mcp/",
                }),
                "{path} {host}"
            );
        }
    }
}

// The authorization server metadata advertises exactly what this server does: S256 PKCE, public
// clients with metadata documents, and `iss` on authorization responses, with nothing of OpenID
// Connect and no dynamic registration. Its issuer is the resource metadata's, byte for byte.
#[tokio::test]
async fn authorization_server_metadata() {
    let harness = Harness::new().await;
    let resp = harness
        .discover("/.well-known/oauth-authorization-server", "mcp.example.com")
        .await;
    assert_metadata_headers(&resp);
    let metadata: Value = resp.json().await.expect("Invalid metadata");
    assert_eq!(
        metadata,
        json!({
            "issuer": OAUTH_ISSUER,
            "authorization_endpoint": format!("{OAUTH_ISSUER}/oauth/authorize"),
            "token_endpoint": format!("{OAUTH_ISSUER}/oauth/token"),
            "revocation_endpoint": format!("{OAUTH_ISSUER}/oauth/revoke"),
            "response_types_supported": ["code"],
            "response_modes_supported": ["query"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "revocation_endpoint_auth_methods_supported": ["none"],
            "client_id_metadata_document_supported": true,
            "authorization_response_iss_parameter_supported": true,
            "scopes_supported": ["projects:read", "projects:write", "offline_access"],
            "service_documentation": "https://bencher.dev/docs/how-to/mcp/",
        })
    );

    let resp = harness
        .discover(
            "/.well-known/oauth-protected-resource/mcp",
            "mcp.example.com",
        )
        .await;
    let resource: Value = resp.json().await.expect("Invalid metadata");
    assert_eq!(resource["authorization_servers"][0], metadata["issuer"]);
}

struct Harness {
    server: TestServer,
    /// A client that does not follow redirects, so every `Location` can be read.
    client: reqwest::Client,
    now: Arc<AtomicI64>,
    /// The address every request comes from, when the test limits requests by address.
    forwarded_for: Option<&'static str>,
}

impl Harness {
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn new() -> Self {
        let json: JsonOAuthServer = serde_json::from_value(config()).expect("Invalid OAuth config");
        Self::with_oauth(
            OAuthServer::try_from(json).expect("Invalid OAuth server"),
            None,
        )
        .await
    }

    /// The public `limits` for each address, every request from the same one.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn rate_limited(
        config: Value,
        lookup: Option<Arc<ScriptedLookup>>,
        limits: Value,
    ) -> Self {
        let json: JsonOAuthServer = serde_json::from_value(config).expect("Invalid OAuth config");
        let oauth = if let Some(lookup) = lookup {
            OAuthServer::with_egress(json, egress(lookup))
        } else {
            OAuthServer::try_from(json)
        }
        .expect("Invalid OAuth server");
        let rate_limiting: JsonRateLimiting =
            serde_json::from_value(json!({ "public": limits })).expect("Invalid rate limiting");
        let mut harness = Self::with_oauth(oauth, Some(RateLimiting::from(rate_limiting))).await;
        harness.forwarded_for = Some("203.0.113.7");
        harness
    }

    /// Rate limited by `rate_limiting`, every request from the same address.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn with_rate_limiting(rate_limiting: Value) -> Self {
        let json: JsonOAuthServer = serde_json::from_value(config()).expect("Invalid OAuth config");
        let rate_limiting: JsonRateLimiting =
            serde_json::from_value(rate_limiting).expect("Invalid rate limiting");
        let mut harness = Self::with_oauth(
            OAuthServer::try_from(json).expect("Invalid OAuth server"),
            Some(RateLimiting::from(rate_limiting)),
        )
        .await;
        harness.forwarded_for = Some("203.0.113.7");
        harness
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn with_oauth(oauth: OAuthServer, rate_limiting: Option<RateLimiting>) -> Self {
        let now = Arc::new(AtomicI64::new(NOW));
        let clock_now = Arc::clone(&now);
        let server = TestServer::new_with_options(TestServerOptions {
            clock: Some(Clock::Custom(Arc::new(move || {
                DateTime::try_from(clock_now.load(Ordering::SeqCst)).expect("Invalid time")
            }))),
            rate_limiting,
            oauth: Some(oauth),
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
            forwarded_for: None,
        }
    }

    fn with_address(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(address) = self.forwarded_for {
            request.header("X-Forwarded-For", address)
        } else {
            request
        }
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn authorize(&self, query: &[(String, String)]) -> reqwest::Response {
        self.with_address(
            self.client
                .get(self.server.api_url("/oauth/authorize"))
                .query(query),
        )
        .send()
        .await
        .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn view(&self, uuid: OAuthAuthorizationUuid) -> reqwest::Response {
        self.with_address(
            self.client.get(
                self.server
                    .api_url(&format!("/v0/oauth/authorizations/{uuid}")),
            ),
        )
        .send()
        .await
        .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn answer(
        &self,
        uuid: OAuthAuthorizationUuid,
        approve: bool,
        bearer: Option<&str>,
    ) -> reqwest::Response {
        let mut request = self
            .client
            .post(
                self.server
                    .api_url(&format!("/v0/oauth/authorizations/{uuid}")),
            )
            .json(&json!({ "approve": approve }));
        if let Some(bearer) = bearer {
            request = request.header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            );
        }
        self.with_address(request)
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn authorization(&self, uuid: OAuthAuthorizationUuid) -> QueryOAuthAuthorization {
        QueryOAuthAuthorization::from_uuid(&mut self.server.db_conn(), uuid)
            .expect("Missing authorization")
    }

    /// A pending request recorded directly, as if its client's document had been fetched.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn pending(&self, client_id: &str, redirect_uri: &str) -> OAuthAuthorizationUuid {
        NewOAuthAuthorization {
            client_id: client_id.parse().expect("Invalid client ID"),
            client_name: "Pending".parse().expect("Invalid client name"),
            redirect_uri: redirect_uri.parse().expect("Invalid redirect URI"),
            scope: "projects:read".parse().expect("Invalid scope"),
            resource: oauth_resource(),
            code_challenge: CHALLENGE.parse().expect("Invalid code challenge"),
            state: None,
            nonce: None,
            max_age: None,
            refresh: false,
        }
        .insert(
            &mut self.server.db_conn(),
            DateTime::try_from(NOW).expect("Invalid time"),
        )
        .expect("Failed to record the request")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn authorizations(&self) -> i64 {
        schema::oauth_authorization::table
            .count()
            .get_result(&mut self.server.db_conn())
            .expect("Failed to count authorizations")
    }

    async fn create_user_key(&self, user: &TestUser) -> (UserKeyUuid, UserKey) {
        self.create_user_key_with_ttl(user, None).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn create_user_key_with_ttl(
        &self,
        user: &TestUser,
        ttl: Option<u32>,
    ) -> (UserKeyUuid, UserKey) {
        let user_slug: &str = user.slug.as_ref();
        let resp = self
            .server
            .client
            .post(self.server.api_url(&format!("/v0/users/{user_slug}/keys")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&json!({"name": "Pasted Key", "ttl": ttl}))
            .send()
            .await
            .expect("Failed to create a user key");
        assert_eq!(resp.status(), StatusCode::CREATED);
        let key: JsonUserKeyCreated = resp.json().await.expect("Invalid user key");
        (key.uuid, key.key)
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn user_key_id(&self, uuid: UserKeyUuid) -> UserKeyId {
        schema::user_key::table
            .filter(schema::user_key::uuid.eq(uuid))
            .select(schema::user_key::id)
            .first(&mut self.server.db_conn())
            .expect("Missing user key")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn user_id(&self, user: &TestUser) -> UserId {
        schema::user::table
            .filter(schema::user::uuid.eq(&user.uuid))
            .select(schema::user::id)
            .first(&mut self.server.db_conn())
            .expect("Missing user")
    }

    fn advance(&self, seconds: i64) {
        self.now.fetch_add(seconds, Ordering::SeqCst);
    }

    /// An authorization code for `request`, approved with `bearer`.
    async fn code(&self, request: &[(String, String)], bearer: &str) -> String {
        let uuid = consent_request(&self.authorize(request).await);
        let url = redirect_url(self.answer(uuid, true, Some(bearer)).await).await;
        param(&query(&url), "code").to_owned()
    }

    /// The access and refresh tokens of a grant the user approved.
    async fn tokens(&self, user: &TestUser) -> (String, String) {
        let code = self.code(&request(), &user.token).await;
        let tokens = token_body(self.token(&exchange(&code)).await).await;
        (
            string(&tokens, "access_token"),
            string(&tokens, "refresh_token"),
        )
    }

    async fn token(&self, params: &[(String, String)]) -> reqwest::Response {
        self.post_form("/oauth/token", params, None).await
    }

    async fn revoke(&self, params: &[(String, String)]) -> reqwest::Response {
        self.post_form("/oauth/revoke", params, None).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn post_form(
        &self,
        path: &str,
        params: &[(String, String)],
        authorization: Option<&str>,
    ) -> reqwest::Response {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();
        let mut request = self
            .client
            .post(self.server.api_url(path))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body);
        if let Some(authorization) = authorization {
            request = request.header(AUTHORIZATION, authorization);
        }
        self.with_address(request)
            .send()
            .await
            .expect("Request failed")
    }

    /// A discovery document, as read at `host`.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn discover(&self, path: &str, host: &str) -> reqwest::Response {
        self.client
            .get(self.server.api_url(path))
            .header(HOST, host)
            .send()
            .await
            .expect("Request failed")
    }

    /// A call to the MCP server's `list_projects` tool for an organization with an access token.
    /// Signup gives every user an organization of their own, with their UUID.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn list_projects(
        &self,
        organization: impl std::fmt::Display,
        access_token: &str,
    ) -> reqwest::Response {
        self.client
            .post(self.server.api_url("/mcp"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(access_token),
            )
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "list_projects",
                    "arguments": {"organization": organization.to_string()},
                },
            }))
            .send()
            .await
            .expect("MCP request failed")
    }

    /// The status of the user's own `GET /v0/users/{user}`, with their session.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn user_status(&self, user: &TestUser) -> StatusCode {
        let user_slug: &str = user.slug.as_ref();
        self.server
            .client
            .get(self.server.api_url(&format!("/v0/users/{user_slug}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Request failed")
            .status()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn revoke_user_key(&self, user: &TestUser, key: UserKeyUuid) {
        let user_slug: &str = user.slug.as_ref();
        let resp = self
            .server
            .client
            .delete(
                self.server
                    .api_url(&format!("/v0/users/{user_slug}/keys/{key}")),
            )
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Failed to revoke a user key");
        assert!(resp.status().is_success(), "{}", resp.status());
    }

    /// The only grant.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn grant(&self) -> QueryOAuthGrant {
        schema::oauth_grant::table
            .first(&mut self.server.db_conn())
            .expect("Missing grant")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn grants(&self) -> i64 {
        schema::oauth_grant::table
            .count()
            .get_result(&mut self.server.db_conn())
            .expect("Failed to count grants")
    }

    /// Every token held, in the order they were issued.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn token_rows(&self) -> Vec<QueryOAuthToken> {
        schema::oauth_token::table
            .order(schema::oauth_token::id)
            .load(&mut self.server.db_conn())
            .expect("Failed to load tokens")
    }

    /// Send a request while holding the API's writer, and apply `change` with it once the request
    /// has had time to pass its read path, so the writer then sees what `change` left. The writer
    /// waits on its lock without a timeout.
    async fn post_racing_the_writer<F>(
        &self,
        path: &str,
        params: &[(String, String)],
        change: F,
    ) -> reqwest::Response
    where
        F: FnOnce(&mut DbConnection),
    {
        let mut writer = Arc::clone(&self.server.context().database.connection)
            .lock_owned()
            .await;
        let (resp, ()) = tokio::join!(self.post_form(path, params, None), async move {
            tokio::time::sleep(READ_PATH).await;
            change(&mut writer);
        });
        resp
    }

    /// Expire a refresh token now.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn expire_token(&self, refresh_token: &str) {
        let refresh_token: OAuthRefreshToken = refresh_token.parse().expect("Invalid token");
        diesel::update(
            schema::oauth_token::table
                .filter(schema::oauth_token::token_hash.eq(OAuthTokenHash::from(&refresh_token))),
        )
        .set(
            schema::oauth_token::expiration
                .eq(DateTime::try_from(self.now.load(Ordering::SeqCst)).expect("Invalid time")),
        )
        .execute(&mut self.server.db_conn())
        .expect("Failed to expire the token");
    }
}

fn config() -> Value {
    json!({
        "issuer": OAUTH_ISSUER,
        "resources": [format!("{OAUTH_ISSUER}/mcp"), "https://mcp.example.com/mcp"],
        "clients": [
            {
                "id": CLIENT_ID,
                "metadata": {
                    "client_id": CLIENT_ID,
                    "client_name": "Example Client",
                    "redirect_uris": [REDIRECT_URI, "https://client.example/callback?tenant=1"],
                    "grant_types": ["authorization_code", "refresh_token"],
                },
            },
            {
                "id": LOOPBACK_CLIENT_ID,
                "metadata": {
                    "client_id": LOOPBACK_CLIENT_ID,
                    "redirect_uris": [
                        "http://localhost/callback",
                        "http://127.0.0.1/callback",
                        "http://[::1]/callback",
                    ],
                },
            },
        ],
    })
}

fn request() -> Vec<(String, String)> {
    [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("scope", "projects:read projects:write"),
        ("state", STATE),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect()
}

/// A valid request with each named parameter set to the value given, or left out.
fn request_with(changes: &[(&str, Option<&str>)]) -> Vec<(String, String)> {
    changed(request(), changes)
}

fn exchange(code: &str) -> Vec<(String, String)> {
    params(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", VERIFIER),
    ])
}

fn exchange_with(code: &str, changes: &[(&str, Option<&str>)]) -> Vec<(String, String)> {
    changed(exchange(code), changes)
}

fn loopback_exchange(code: &str, redirect_uri: &str) -> Vec<(String, String)> {
    exchange_with(
        code,
        &[
            ("client_id", Some(LOOPBACK_CLIENT_ID)),
            ("redirect_uri", Some(redirect_uri)),
        ],
    )
}

fn refresh(refresh_token: &str) -> Vec<(String, String)> {
    params(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ])
}

fn refresh_with(refresh_token: &str, changes: &[(&str, Option<&str>)]) -> Vec<(String, String)> {
    changed(refresh(refresh_token), changes)
}

fn params(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// `params` with each named parameter set to the value given, or left out.
fn changed(
    mut params: Vec<(String, String)>,
    changes: &[(&str, Option<&str>)],
) -> Vec<(String, String)> {
    for (name, value) in changes {
        params.retain(|(key, _)| key != name);
        if let Some(value) = value {
            params.push(((*name).to_owned(), (*value).to_owned()));
        }
    }
    params
}

fn loopback_request(redirect_uri: &str) -> Vec<(String, String)> {
    request_with(&[
        ("client_id", Some(LOOPBACK_CLIENT_ID)),
        ("redirect_uri", Some(redirect_uri)),
    ])
}

fn one_attempt() -> Value {
    json!({"attempts": {"minute": 1}})
}

#[expect(clippy::expect_used, reason = "test helper")]
fn error_page(error: &str) -> Url {
    Url::parse(&format!("{CONSOLE_URL}/auth/authorize?error={error}")).expect("Invalid URL")
}

/// Answers each lookup it has an address for, and fails the rest, recording every name.
struct ScriptedLookup {
    answers: HashMap<String, IpAddr>,
    names: Mutex<Vec<String>>,
}

impl ScriptedLookup {
    #[expect(clippy::expect_used, reason = "test helper")]
    fn new(answers: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers
                .iter()
                .map(|(host, address)| ((*host).to_owned(), address.parse().expect("Invalid IP")))
                .collect(),
            names: Mutex::new(Vec::new()),
        })
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn names(&self) -> Vec<String> {
        self.names.lock().expect("Poisoned").clone()
    }
}

#[async_trait]
impl Lookup for ScriptedLookup {
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn lookup(&self, host: &str) -> io::Result<Vec<IpAddr>> {
        self.names.lock().expect("Poisoned").push(host.to_owned());
        self.answers
            .get(host)
            .map(|address| vec![*address])
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

/// The production egress policy, with lookups answered by `lookup`.
fn egress(lookup: Arc<ScriptedLookup>) -> EgressClient {
    // The test server installs the TLS crypto provider only once it starts.
    let _provider = aws_lc_rs::default_provider().install_default();
    EgressClient::with_lookup(
        EgressPolicy::Public,
        EgressTimeouts {
            connect: Duration::from_secs(5),
            total: Duration::from_secs(5),
        },
        lookup,
    )
}

#[expect(clippy::expect_used, reason = "test helper")]
fn location(resp: &reqwest::Response) -> Url {
    assert_eq!(resp.status(), StatusCode::FOUND);
    Url::parse(
        resp.headers()
            .get(LOCATION)
            .expect("Missing Location")
            .to_str()
            .expect("Invalid Location"),
    )
    .expect("Invalid Location URL")
}

/// The pending request a response sends the browser to the consent page for.
#[expect(clippy::expect_used, reason = "test helper")]
fn consent_request(resp: &reqwest::Response) -> OAuthAuthorizationUuid {
    let location = location(resp);
    assert_eq!(
        location.as_str().split('?').next(),
        Some(&*format!("{CONSOLE_URL}/auth/authorize"))
    );
    let params = query(&location);
    assert_eq!(params.len(), 1, "{location}");
    param(&params, "request")
        .parse()
        .expect("Invalid request UUID")
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn redirect_url(resp: reqwest::Response) -> Url {
    let redirect: JsonOAuthRedirect = resp.json().await.expect("Invalid redirect");
    redirect.url
}

fn query(url: &Url) -> Vec<(String, String)> {
    url.query_pairs().into_owned().collect()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn param<'p>(params: &'p [(String, String)], name: &str) -> &'p str {
    params
        .iter()
        .find_map(|(key, value)| (key == name).then_some(value.as_str()))
        .expect("Missing parameter")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn header<'r>(resp: &'r reqwest::Response, name: &str) -> &'r str {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .expect("Missing header")
}

/// Assert a successful token response, which is never cached, and return it.
#[expect(clippy::expect_used, reason = "test helper")]
async fn token_body(resp: reqwest::Response) -> Value {
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    assert_eq!(header(&resp, "access-control-allow-origin"), "*");
    let tokens: Value = resp.json().await.expect("Invalid token response");
    assert_eq!(tokens.get("token_type"), Some(&json!("Bearer")), "{tokens}");
    tokens
}

/// Assert an OAuth error with exactly an error code and a description, never cached,
/// and return the body.
#[expect(clippy::expect_used, reason = "test helper")]
async fn assert_oauth_error(resp: reqwest::Response, status: StatusCode, error: &str) -> String {
    assert_eq!(resp.status(), status);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    assert_eq!(header(&resp, "content-type"), "application/json");
    assert_eq!(header(&resp, "access-control-allow-origin"), "*");
    let body = resp.text().await.expect("Invalid body");
    let fields: serde_json::Map<String, Value> =
        serde_json::from_str(&body).expect("Invalid error body");
    assert_eq!(fields.len(), 2, "{body}");
    assert_eq!(fields.get("error"), Some(&json!(error)), "{body}");
    assert!(
        fields
            .get("error_description")
            .and_then(Value::as_str)
            .is_some_and(|description| !description.is_empty()),
        "{body}"
    );
    body
}

fn assert_metadata_headers(resp: &reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(resp, "cache-control"), "public, max-age=300");
    assert_eq!(header(resp, "access-control-allow-origin"), "*");
    assert_eq!(header(resp, "content-type"), "application/json");
}

#[expect(clippy::expect_used, reason = "test helper")]
fn string(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .expect("Missing string")
        .to_owned()
}

/// The path of an advertised endpoint, which is on the issuer.
#[expect(clippy::expect_used, reason = "test helper")]
fn advertised_path(metadata: &Value, name: &str) -> String {
    string(metadata, name)
        .strip_prefix(OAUTH_ISSUER)
        .expect("Endpoint not on the issuer")
        .to_owned()
}

/// The slugs of the projects a successful `list_projects` call listed.
#[expect(clippy::expect_used, reason = "test helper")]
async fn project_slugs(resp: reqwest::Response) -> Vec<String> {
    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    assert_ne!(
        message.pointer("/result/isError"),
        Some(&Value::Bool(true)),
        "{message}"
    );
    message
        .pointer("/result/structuredContent/data")
        .and_then(Value::as_array)
        .expect("Missing projects")
        .iter()
        .filter_map(|project| project.get("slug").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn delete_token(conn: &mut DbConnection, token_hash: &OAuthTokenHash) {
    diesel::delete(
        schema::oauth_token::table.filter(schema::oauth_token::token_hash.eq(token_hash)),
    )
    .execute(conn)
    .expect("Failed to delete the token");
}
