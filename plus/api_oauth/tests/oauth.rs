#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the OAuth authorization endpoint and the consent API.

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
    OAuthAuthorizationUuid, OAuthCode, OAuthCodeHash, OAuthScope, UserKey, UserKeyUuid,
    system::config::{JsonOAuthServer, JsonRateLimiting},
};
use bencher_schema::{
    context::{OAuthServer, RateLimiting},
    model::user::{
        UserId,
        key::UserKeyId,
        oauth_authorization::{NewOAuthAuthorization, OAuthAnswer, QueryOAuthAuthorization},
    },
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{Method, StatusCode, header::LOCATION};
use rustls::crypto::aws_lc_rs;
use serde_json::{Value, json};
use url::{Url, form_urlencoded};

/// The time every test server here starts at.
const NOW: i64 = 1_800_000_000;
const CONSOLE_URL: &str = "http://localhost:3000";
const CLIENT_ID: &str = "https://client.example/oauth/client.json";
const REDIRECT_URI: &str = "https://client.example/callback";
const LOOPBACK_CLIENT_ID: &str = "https://cli.example/oauth/client.json";
/// Configured without a document, so it is fetched.
const FETCHED_CLIENT_ID: &str = "https://fetched.example/oauth/client.json";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const SHORT_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-c";
const STATE: &str = "af0ifjsldkj";

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
        ),
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
    let harness = Harness::with_oauth(OAuthServer::from(json), None).await;
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
        Self::with_oauth(OAuthServer::from(json), None).await
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
            OAuthServer::from(json)
        };
        let rate_limiting: JsonRateLimiting =
            serde_json::from_value(json!({ "public": limits })).expect("Invalid rate limiting");
        let mut harness = Self::with_oauth(oauth, Some(RateLimiting::from(rate_limiting))).await;
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

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn create_user_key(&self, user: &TestUser) -> (UserKeyUuid, UserKey) {
        let user_slug: &str = user.slug.as_ref();
        let resp = self
            .server
            .client
            .post(self.server.api_url(&format!("/v0/users/{user_slug}/keys")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&json!({"name": "Pasted Key"}))
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
    let mut request = request();
    for (name, value) in changes {
        request.retain(|(key, _)| key != name);
        if let Some(value) = value {
            request.push(((*name).to_owned(), (*value).to_owned()));
        }
    }
    request
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
