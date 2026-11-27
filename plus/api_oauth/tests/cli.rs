#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the Bencher CLI's built-in OAuth client: the authorization server's
//! metadata, the authorization request, consent, the token and revocation endpoints, and what its
//! access tokens can reach, on a server with `plus.oauth` and on one without.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use bencher_api_tests::{TestServer, TestServerOptions, TestUser, helpers::OAUTH_SIGNING_KEY};
use bencher_json::{
    Clock, DateTime, JsonOAuthRedirect, OAuthAuthorizationUuid, system::config::JsonOAuthServer,
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{
    StatusCode,
    header::{CONTENT_TYPE, COOKIE, HOST as HOST_HEADER, LOCATION},
};
use serde_json::{Value, json};
use url::{Url, form_urlencoded};

/// The time every test server here starts at.
const NOW: i64 = 1_800_000_000;
const CONSOLE_URL: &str = "http://localhost:3000";
const CLI: &str = "bencher_cli";
const LOOPBACK: &str = "http://127.0.0.1:53682/callback";
const CODE_PAGE: &str = "http://localhost:3000/auth/cli";
const DEVICE: &str = "ada-macbook";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
/// The code verifier `CHALLENGE` was made from (RFC 7636, appendix B).
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const OTHER_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXl";
const STATE: &str = "af0ifjsldkj";
const OUTSIDE_CLIENT: &str = "https://client.example/oauth/client.json";
const OUTSIDE_REDIRECT: &str = "https://client.example/callback";
/// The refusal of the Bencher CLI's token at the MCP server.
const CLI_AT_MCP: &str = "accepted only by the REST API";
/// Past the fresh sign-in window.
const STALE: i64 = 11 * 60;

// The whole flow on a server with no `plus.oauth`: the metadata names the server itself, the CLI
// is approved, exchanges its code, calls the REST API, refreshes, and is revoked.
#[tokio::test]
async fn cli_signs_in_without_oauth_config() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;

    let resp = harness.get("/.well-known/oauth-authorization-server").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(header(&resp, "cache-control"), "no-store");
    let metadata: Value = resp.json().await.expect("Invalid metadata");
    let issuer = harness.server.api_url("");
    let issuer = issuer.trim_end_matches('/');
    assert_eq!(metadata["issuer"], issuer);
    assert_eq!(
        metadata["authorization_endpoint"],
        format!("{issuer}/oauth/authorize")
    );
    assert_eq!(metadata["token_endpoint"], format!("{issuer}/oauth/token"));
    assert_eq!(
        metadata["revocation_endpoint"],
        format!("{issuer}/oauth/revoke")
    );
    assert_eq!(
        metadata["code_challenge_methods_supported"],
        json!(["S256"])
    );
    assert_eq!(metadata["client_id_metadata_document_supported"], false);
    assert_eq!(
        metadata["authorization_response_iss_parameter_supported"],
        false
    );
    assert_eq!(metadata["scopes_supported"], json!(["admin", "user"]));

    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let resp = harness.view(uuid).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let view: Value = resp.json().await.expect("Invalid authorization");
    assert_eq!(view["client"]["id"], CLI);
    assert_eq!(view["client"]["name"], "Bencher CLI");
    assert_eq!(view["client"]["verified"], true);
    assert_eq!(view["device"], DEVICE);
    assert_eq!(view["redirect"]["loopback"], true);
    assert_eq!(view["scopes"], json!(["user"]));

    let resp = harness.approve(uuid, &harness.fresh_cookie(&user)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let url = redirect_url(resp).await;
    assert!(url.as_str().starts_with(&format!("{LOOPBACK}?")), "{url}");
    let answer = query(&url);
    assert_eq!(param(&answer, "state"), STATE);
    assert!(!answer.iter().any(|(name, _)| name == "iss"), "{url}");
    let code = param(&answer, "code").to_owned();

    let tokens = harness.exchange(&code, VERIFIER, LOOPBACK).await;
    assert_eq!(tokens["token_type"], "Bearer");
    assert_eq!(tokens["expires_in"], 3600);
    assert_eq!(tokens["scope"], "user");
    let access = string(&tokens, "access_token");
    let refresh = string(&tokens, "refresh_token");
    assert_eq!(harness.rest_user(&user, &access).await, StatusCode::OK);

    let resp = harness.token(&refresh_params(&refresh)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let refreshed: Value = resp.json().await.expect("Invalid tokens");
    let access = string(&refreshed, "access_token");
    let refresh = string(&refreshed, "refresh_token");
    assert_eq!(harness.rest_user(&user, &access).await, StatusCode::OK);

    let resp = harness
        .post_form(
            "/oauth/revoke",
            &params(&[("token", refresh.as_str()), ("client_id", CLI)]),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        harness.rest_user(&user, &access).await,
        StatusCode::UNAUTHORIZED
    );
    let resp = harness.token(&refresh_params(&refresh)).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// Kills the CLI's client refused on plain HTTP off loopback, where a server without `plus.oauth`
// may serve no TLS, or its grant unreadable once stored.
#[tokio::test]
async fn cli_signs_in_over_plain_http_on_any_host() {
    const HOST: &str = "bencher.lan:6610";
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let metadata: Value = harness
        .client
        .get(
            harness
                .server
                .api_url("/.well-known/oauth-authorization-server"),
        )
        .header(HOST_HEADER, HOST)
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Invalid metadata");
    assert_eq!(metadata["issuer"], "http://bencher.lan:6610");
    assert_eq!(
        metadata["token_endpoint"],
        "http://bencher.lan:6610/oauth/token"
    );

    let resp = harness
        .client
        .get(harness.server.api_url("/oauth/authorize"))
        .header(HOST_HEADER, HOST)
        .query(&cli_request())
        .send()
        .await
        .expect("Request failed");
    let uuid = consent_request(&resp);
    let url = redirect_url(harness.approve(uuid, &harness.fresh_cookie(&user)).await).await;
    let tokens = harness
        .exchange(param(&query(&url), "code"), VERIFIER, LOOPBACK)
        .await;
    assert_eq!(
        harness
            .rest_user(&user, &string(&tokens, "access_token"))
            .await,
        StatusCode::OK
    );
    let refreshed = harness
        .token(&refresh_params(&string(&tokens, "refresh_token")))
        .await;
    assert_eq!(refreshed.status(), StatusCode::OK);
}

// Kills a server without `plus.oauth` admitting an outside app along with the CLI, or serving the
// MCP server's metadata.
#[tokio::test]
async fn outside_app_is_refused_without_oauth_config() {
    let harness = Harness::new(false).await;
    let resp = harness
        .authorize(&[
            ("response_type", "code"),
            ("client_id", OUTSIDE_CLIENT),
            ("redirect_uri", OUTSIDE_REDIRECT),
            ("code_challenge", CHALLENGE),
            ("code_challenge_method", "S256"),
        ])
        .await;
    assert_eq!(location(&resp), error_page("invalid_client"));
    assert_eq!(harness.authorizations(), 0);
    for path in [
        "/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource/mcp",
        "/.well-known/openid-configuration",
        "/oauth/jwks.json",
    ] {
        assert_eq!(
            harness.get(path).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

// Kills an outside app granted the Bencher CLI's scopes, at its request or at a refresh.
#[tokio::test]
async fn outside_app_never_gets_cli_scopes() {
    // A server that signs ID tokens grants scopes beyond the projects', so only the CLI's own
    // rule keeps its scopes from an outside app.
    let mut config = oauth_config();
    config["signing_key"] = OAUTH_SIGNING_KEY.into();
    let harness = Harness::with_options(Some(config), false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let uuid = consent_request(
        &harness
            .authorize(&[
                ("response_type", "code"),
                ("client_id", OUTSIDE_CLIENT),
                ("redirect_uri", OUTSIDE_REDIRECT),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
                ("scope", "user admin"),
            ])
            .await,
    );
    let view: Value = harness.view(uuid).await.json().await.expect("Invalid view");
    assert_eq!(view["scopes"], json!(["projects:read"]));
    let url = redirect_url(harness.approve(uuid, &harness.fresh_cookie(&user)).await).await;
    let resp = harness
        .token(&params(&[
            ("grant_type", "authorization_code"),
            ("code", param(&query(&url), "code")),
            ("client_id", OUTSIDE_CLIENT),
            ("redirect_uri", OUTSIDE_REDIRECT),
            ("code_verifier", VERIFIER),
        ]))
        .await;
    let tokens: Value = resp.json().await.expect("Invalid tokens");
    assert_eq!(tokens["scope"], "projects:read");
    let resp = harness
        .token(&params(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &string(&tokens, "refresh_token")),
            ("client_id", OUTSIDE_CLIENT),
            ("scope", "admin"),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let refreshed: Value = resp.json().await.expect("Invalid tokens");
    assert_eq!(refreshed["scope"], "projects:read");
}

// On a server with `plus.oauth`, the outside apps' metadata stays as it is, and the CLI signs in
// beside them, with `iss` from the configured issuer.
#[tokio::test]
async fn cli_signs_in_beside_outside_apps() {
    let harness = Harness::new(true).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let metadata: Value = harness
        .get("/.well-known/oauth-authorization-server")
        .await
        .json()
        .await
        .expect("Invalid metadata");
    assert_eq!(metadata["issuer"], "https://api.example.com");
    assert_eq!(metadata["client_id_metadata_document_supported"], true);
    assert_eq!(
        metadata["scopes_supported"],
        json!(["projects:read", "projects:write", "offline_access"])
    );

    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let url = redirect_url(harness.approve(uuid, &harness.fresh_cookie(&user)).await).await;
    let params = query(&url);
    assert_eq!(param(&params, "iss"), "https://api.example.com");
    let tokens = harness
        .exchange(param(&params, "code"), VERIFIER, LOOPBACK)
        .await;
    assert_eq!(
        harness
            .rest_user(&user, &string(&tokens, "access_token"))
            .await,
        StatusCode::OK
    );
}

// Kills a redirect the CLI client did not register: a loopback address on any port, or the
// console's own code page, and nothing else.
#[tokio::test]
async fn cli_redirects_are_loopback_or_the_code_page() {
    let harness = Harness::new(false).await;
    for redirect_uri in [
        "http://127.0.0.1:53682/callback",
        "http://127.0.0.1:1/",
        "http://127.0.0.1:65535/any/path",
        "http://[::1]:53682/callback",
        CODE_PAGE,
    ] {
        let resp = harness
            .authorize(&cli_request_with(&[("redirect_uri", Some(redirect_uri))]))
            .await;
        consent_request(&resp);
    }
    for redirect_uri in [
        "http://localhost:53682/callback",
        "https://127.0.0.1:53682/callback",
        "http://127.0.0.2:53682/callback",
        "https://client.example/callback",
        "http://localhost:3001/auth/cli",
        "https://localhost:3000/auth/cli",
        "http://evil.example/auth/cli",
        "http://localhost:3000/auth/cli/",
        "http://localhost:3000/auth/cli?next=1",
        "http://localhost:3000/auth/authorize",
    ] {
        let resp = harness
            .authorize(&cli_request_with(&[("redirect_uri", Some(redirect_uri))]))
            .await;
        assert_eq!(
            location(&resp),
            error_page("invalid_redirect_uri"),
            "{redirect_uri}"
        );
    }
    let resp = harness
        .authorize(&cli_request_with(&[("redirect_uri", None)]))
        .await;
    assert_eq!(location(&resp), error_page("invalid_request"));
}

// Kills a CLI request without PKCE S256, and an exchange with the wrong verifier.
#[tokio::test]
async fn cli_needs_pkce_s256() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    for changes in [
        vec![("code_challenge_method", None)],
        vec![("code_challenge_method", Some("plain"))],
        vec![("code_challenge", None)],
        vec![("code_challenge", Some("short"))],
    ] {
        let resp = harness.authorize(&cli_request_with(&changes)).await;
        let url = location(&resp);
        assert!(url.as_str().starts_with(&format!("{LOOPBACK}?")), "{url}");
        assert_eq!(param(&query(&url), "error"), "invalid_request", "{url}");
    }

    // The code page is the console's, so a refusal shows on the consent page and never there.
    let resp = harness
        .authorize(&cli_request_with(&[
            ("redirect_uri", Some(CODE_PAGE)),
            ("code_challenge_method", None),
        ]))
        .await;
    assert_eq!(location(&resp), error_page("invalid_request"));

    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let url = redirect_url(harness.approve(uuid, &harness.fresh_cookie(&user)).await).await;
    let code = param(&query(&url), "code").to_owned();
    let resp = harness
        .token(&params(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", CLI),
            ("redirect_uri", LOOPBACK),
            ("code_verifier", OTHER_VERIFIER),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.expect("Invalid error");
    assert_eq!(body["error"], "invalid_grant");
}

// Kills a CLI request with no device, or one that is not a hostname.
#[tokio::test]
async fn cli_names_its_device() {
    let harness = Harness::new(false).await;
    let longest = "a".repeat(64);
    for device in [
        "ada-macbook",
        "Adas-MacBook-Pro.local",
        "build_01",
        &longest,
    ] {
        let uuid = consent_request(
            &harness
                .authorize(&cli_request_with(&[("device", Some(device))]))
                .await,
        );
        let view: Value = harness.view(uuid).await.json().await.expect("Invalid view");
        assert_eq!(view["device"], device);
    }
    let too_long = "a".repeat(65);
    for device in [
        None,
        Some(""),
        Some("-ada"),
        Some(".ada"),
        Some("ada macbook"),
        Some("ada<b>"),
        Some("ada\u{e9}"),
        Some(too_long.as_str()),
    ] {
        let resp = harness
            .authorize(&cli_request_with(&[("device", device)]))
            .await;
        let url = location(&resp);
        assert!(url.as_str().starts_with(&format!("{LOOPBACK}?")), "{url}");
        assert_eq!(
            param(&query(&url), "error"),
            "invalid_request",
            "{device:?}"
        );
    }
}

// Kills consent to the CLI on a session alone: it needs a sign-in within the fresh sign-in window,
// while an outside app's consent takes any live session.
#[tokio::test]
async fn cli_consent_needs_a_fresh_sign_in() {
    let harness = Harness::new(true).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let cookie = harness.fresh_cookie(&user);
    harness.advance(STALE);

    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let resp = harness.approve(uuid, &cookie).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body: Value = resp.json().await.expect("Invalid error");
    assert_eq!(body["error_code"], "fresh_sign_in");
    let resp = harness.view(uuid).await;
    assert_eq!(resp.status(), StatusCode::OK, "the request still waits");

    let resp = harness.approve(uuid, &harness.fresh_cookie(&user)).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let outside = consent_request(
        &harness
            .authorize(&[
                ("response_type", "code"),
                ("client_id", OUTSIDE_CLIENT),
                ("redirect_uri", OUTSIDE_REDIRECT),
                ("code_challenge", CHALLENGE),
                ("code_challenge_method", "S256"),
            ])
            .await,
    );
    let resp = harness.approve(outside, &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// Kills a user key approving the CLI, pasted or as a bearer: only a browser session can.
#[tokio::test]
async fn cli_consent_needs_a_session() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let resp = harness
        .decide(uuid, &json!({"approve": true, "key": user.token}), &[])
        .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp = harness
        .decide(
            uuid,
            &json!({"approve": true}),
            &[(
                bencher_json::AUTHORIZATION,
                &bencher_json::bearer_header(&user.token),
            )],
        )
        .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(harness.view(uuid).await.status(), StatusCode::OK);
}

// Kills an admin grant approved by a reader who is not a server admin, server admin power from a
// grant that did not ask for it, and an admin grant that keeps it once the user loses it.
#[tokio::test]
async fn cli_admin_grant_follows_the_user() {
    let harness = Harness::new(false).await;
    let admin = harness.server.signup("Ada", "ada@example.com").await;
    let other = harness.server.signup("Grace", "grace@example.com").await;
    let admin_request = cli_request_with(&[("scope", Some("admin"))]);

    let uuid = consent_request(&harness.authorize(&admin_request).await);
    let view: Value = harness.view(uuid).await.json().await.expect("Invalid view");
    assert_eq!(view["scopes"], json!(["admin", "user"]));
    let resp = harness.approve(uuid, &harness.fresh_cookie(&other)).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(harness.view(uuid).await.status(), StatusCode::OK);

    let plain = harness.tokens(&admin, &cli_request()).await;
    assert_eq!(plain["scope"], "user");
    let plain = string(&plain, "access_token");
    assert_eq!(harness.rest_user(&admin, &plain).await, StatusCode::OK);
    assert_eq!(
        harness.rest_admin(&plain).await,
        StatusCode::FORBIDDEN,
        "a grant without admin has no server admin power"
    );

    let tokens = harness.tokens(&admin, &admin_request).await;
    assert_eq!(tokens["scope"], "admin user");
    let access = string(&tokens, "access_token");
    assert_eq!(harness.rest_admin(&access).await, StatusCode::OK);

    harness.set_admin(&admin, false);
    assert_eq!(
        harness.rest_admin(&access).await,
        StatusCode::FORBIDDEN,
        "the grant loses admin power with the user"
    );
    harness.set_admin(&admin, true);
    assert_eq!(harness.rest_admin(&access).await, StatusCode::OK);
}

// Kills the CLI's access token doing what needs a browser session (new keys, an email change, a
// plan change, and invites), reaching the MCP server, or an outside app's reaching REST.
#[tokio::test]
async fn cli_token_reaches_rest_but_not_session_actions() {
    let harness = Harness::with_cloud_routes(true).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let slug: &str = user.slug.as_ref();
    let tokens = harness.tokens(&user, &cli_request()).await;
    let access = string(&tokens, "access_token");
    let refresh = string(&tokens, "refresh_token");
    assert_eq!(harness.rest_user(&user, &access).await, StatusCode::OK);

    let org = harness.server.create_org(&user, "Ada Org").await;
    let org_slug: &str = org.slug.as_ref();
    for (method, path, body) in [
        (
            http::Method::POST,
            format!("/v0/users/{slug}/keys"),
            json!({"name": "From the CLI"}),
        ),
        (
            http::Method::PATCH,
            format!("/v0/users/{slug}"),
            json!({"email": "ada.new@example.com"}),
        ),
        (
            http::Method::POST,
            format!("/v0/organizations/{org_slug}/members"),
            json!({"email": "grace@example.com", "role": "leader"}),
        ),
        (
            http::Method::PATCH,
            format!("/v0/organizations/{org_slug}/plan"),
            json!({"cancel_at_period_end": true}),
        ),
    ] {
        let resp = harness
            .server
            .client
            .request(method.clone(), harness.server.api_url(&path))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&access),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {path}");
        let body: Value = resp.json().await.expect("Invalid error");
        assert!(
            body["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("Only a user session")),
            "{method} {path}: {body}"
        );
    }

    // Neither a refresh token nor the MCP server takes the place of the access token at REST.
    assert_eq!(
        harness.rest_user(&user, &refresh).await,
        StatusCode::UNAUTHORIZED
    );
    let resp = harness.mcp(&access).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.text()
            .await
            .is_ok_and(|body| body.contains(CLI_AT_MCP)),
        "the refusal says where the token belongs"
    );

    // An outside app's access token stays at the MCP server.
    let outside = harness.outside_tokens(&user).await;
    assert_eq!(
        harness.rest_user(&user, &outside).await,
        StatusCode::UNAUTHORIZED
    );
}

// Kills the CLI's access token reaching the MCP server on a server without `plus.oauth`, where
// the MCP server takes only Bencher's own keys.
#[tokio::test]
async fn cli_token_is_refused_at_mcp_without_oauth_config() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let tokens = harness.tokens(&user, &cli_request()).await;
    let resp = harness.mcp(&string(&tokens, "access_token")).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.text()
            .await
            .is_ok_and(|body| body.contains(CLI_AT_MCP))
    );
}

// Kills the CLI's access token answered at the user info endpoint, on a server that allows any
// client and serves the CLI's resource, which leaves only the CLI's own rule to refuse it.
#[tokio::test]
async fn cli_token_is_refused_at_userinfo() {
    let config = json!({
        "issuer": "https://api.example.com",
        "resources": ["https://api.example.com/mcp", "https://api.example.com"],
        "signing_key": OAUTH_SIGNING_KEY,
    });
    let harness = Harness::with_options(Some(config), false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let tokens = harness.tokens(&user, &cli_request()).await;
    let resp = harness
        .client
        .get(harness.server.api_url("/oauth/userinfo"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(string(&tokens, "access_token")),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = resp.json().await.expect("Invalid error");
    assert_eq!(body["error"], "invalid_token");
}

// Kills a reused refresh token or code that leaves the CLI's grant alive.
#[tokio::test]
async fn cli_reuse_revokes_its_grant() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let tokens = harness.tokens(&user, &cli_request()).await;
    let first = string(&tokens, "refresh_token");
    let rotated: Value = harness
        .token(&refresh_params(&first))
        .await
        .json()
        .await
        .expect("Invalid tokens");
    let access = string(&rotated, "access_token");
    assert_eq!(harness.rest_user(&user, &access).await, StatusCode::OK);
    let resp = harness.token(&refresh_params(&first)).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        harness.rest_user(&user, &access).await,
        StatusCode::UNAUTHORIZED,
        "the reuse revoked the grant"
    );

    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let url = redirect_url(harness.approve(uuid, &harness.fresh_cookie(&user)).await).await;
    let code = param(&query(&url), "code").to_owned();
    let tokens = harness.exchange(&code, VERIFIER, LOOPBACK).await;
    let access = string(&tokens, "access_token");
    let resp = harness
        .token(&params(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", CLI),
            ("redirect_uri", LOOPBACK),
            ("code_verifier", VERIFIER),
        ]))
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        harness.rest_user(&user, &access).await,
        StatusCode::UNAUTHORIZED
    );
}

// The CLI's grant lasts a year, its access token an hour.
#[tokio::test]
async fn cli_grant_lasts_a_year() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let tokens = harness.tokens(&user, &cli_request()).await;
    let access = string(&tokens, "access_token");
    harness.advance(60 * 60);
    assert_eq!(
        harness.rest_user(&user, &access).await,
        StatusCode::UNAUTHORIZED
    );
    let expiration: i64 = schema::oauth_grant::table
        .select(schema::oauth_grant::expiration)
        .first(&mut harness.server.db_conn())
        .expect("Missing grant");
    assert_eq!(expiration, NOW + 365 * 24 * 60 * 60);
}

// Connected Apps lists the CLI's grant with its device, on a server without `plus.oauth`, and
// revoking it there ends its tokens.
#[tokio::test]
async fn connected_apps_lists_and_revokes_the_cli() {
    let harness = Harness::new(false).await;
    let user = harness.server.signup("Ada", "ada@example.com").await;
    let slug: &str = user.slug.as_ref();
    let tokens = harness.tokens(&user, &cli_request()).await;
    let access = string(&tokens, "access_token");

    let resp = harness
        .server
        .client
        .get(
            harness
                .server
                .api_url(&format!("/v0/users/{slug}/oauth/grants")),
        )
        .headers(harness.server.fresh_session(&user))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let grants: Value = resp.json().await.expect("Invalid grants");
    assert_eq!(grants.as_array().map(Vec::len), Some(1));
    let grant = &grants[0];
    assert_eq!(grant["client"]["id"], CLI);
    assert_eq!(grant["client"]["name"], "Bencher CLI");
    assert_eq!(grant["device"], DEVICE);
    assert_eq!(grant["scopes"], json!(["user"]));

    let uuid = string(grant, "uuid");
    let resp = harness
        .server
        .client
        .delete(
            harness
                .server
                .api_url(&format!("/v0/users/{slug}/oauth/grants/{uuid}")),
        )
        .headers(harness.server.fresh_session(&user))
        .send()
        .await
        .expect("Request failed");
    assert!(resp.status().is_success(), "{}", resp.status());
    assert_eq!(
        harness.rest_user(&user, &access).await,
        StatusCode::UNAUTHORIZED
    );
}

// Deny sends the CLI's own listener `access_denied` with its state, and needs no credential.
#[tokio::test]
async fn cli_deny_returns_access_denied() {
    let harness = Harness::new(false).await;
    let uuid = consent_request(&harness.authorize(&cli_request()).await);
    let resp = harness.decide(uuid, &json!({"approve": false}), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let url = redirect_url(resp).await;
    assert!(url.as_str().starts_with(&format!("{LOOPBACK}?")), "{url}");
    let answer = query(&url);
    assert_eq!(param(&answer, "error"), "access_denied");
    assert_eq!(param(&answer, "state"), STATE);
}

struct Harness {
    server: TestServer,
    /// A client that does not follow redirects, so every `Location` can be read.
    client: reqwest::Client,
    now: Arc<AtomicI64>,
}

impl Harness {
    /// A server with `plus.oauth` for the outside app, or without it.
    async fn new(oauth: bool) -> Self {
        Self::with_options(oauth.then(oauth_config), false).await
    }

    async fn with_cloud_routes(oauth: bool) -> Self {
        Self::with_options(oauth.then(oauth_config), true).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn with_options(oauth: Option<Value>, cloud_routes: bool) -> Self {
        let now = Arc::new(AtomicI64::new(NOW));
        let clock_now = Arc::clone(&now);
        let oauth = oauth.map(|config| {
            serde_json::from_value::<JsonOAuthServer>(config)
                .expect("Invalid OAuth config")
                .try_into()
                .expect("Invalid OAuth server")
        });
        let server = TestServer::new_with_options(TestServerOptions {
            clock: Some(Clock::Custom(Arc::new(move || {
                DateTime::try_from(clock_now.load(Ordering::SeqCst)).expect("Invalid time")
            }))),
            oauth,
            cloud_routes,
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

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(self.server.api_url(path))
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn authorize(&self, query: &[(&str, &str)]) -> reqwest::Response {
        self.client
            .get(self.server.api_url("/oauth/authorize"))
            .query(query)
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn view(&self, uuid: OAuthAuthorizationUuid) -> reqwest::Response {
        self.client
            .get(
                self.server
                    .api_url(&format!("/v0/oauth/authorizations/{uuid}")),
            )
            .send()
            .await
            .expect("Request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn decide(
        &self,
        uuid: OAuthAuthorizationUuid,
        body: &Value,
        headers: &[(&str, &str)],
    ) -> reqwest::Response {
        let mut request = self
            .client
            .post(
                self.server
                    .api_url(&format!("/v0/oauth/authorizations/{uuid}")),
            )
            .json(body);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.send().await.expect("Request failed")
    }

    /// A session cookie the user signed in to now.
    fn fresh_cookie(&self, user: &TestUser) -> String {
        bencher_api_tests::session_cookie(&self.server.session(user))
    }

    async fn approve(&self, uuid: OAuthAuthorizationUuid, cookie: &str) -> reqwest::Response {
        self.decide(
            uuid,
            &json!({"approve": true}),
            &[(COOKIE.as_str(), cookie), ("Sec-Fetch-Site", "same-origin")],
        )
        .await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn post_form(&self, path: &str, params: &[(String, String)]) -> reqwest::Response {
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params)
            .finish();
        self.client
            .post(self.server.api_url(path))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Request failed")
    }

    async fn token(&self, params: &[(String, String)]) -> reqwest::Response {
        self.post_form("/oauth/token", params).await
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn exchange(&self, code: &str, verifier: &str, redirect_uri: &str) -> Value {
        let resp = self
            .token(&params(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", CLI),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
            ]))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        resp.json().await.expect("Invalid tokens")
    }

    /// The tokens of a CLI grant the user approved for `request` in a fresh session.
    async fn tokens(&self, user: &TestUser, request: &[(&str, &str)]) -> Value {
        let uuid = consent_request(&self.authorize(request).await);
        let resp = self.approve(uuid, &self.fresh_cookie(user)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let url = redirect_url(resp).await;
        let code = param(&query(&url), "code").to_owned();
        self.exchange(&code, VERIFIER, LOOPBACK).await
    }

    /// An outside app's access token for the user.
    async fn outside_tokens(&self, user: &TestUser) -> String {
        let uuid = consent_request(
            &self
                .authorize(&[
                    ("response_type", "code"),
                    ("client_id", OUTSIDE_CLIENT),
                    ("redirect_uri", OUTSIDE_REDIRECT),
                    ("code_challenge", CHALLENGE),
                    ("code_challenge_method", "S256"),
                ])
                .await,
        );
        let url = redirect_url(self.approve(uuid, &self.fresh_cookie(user)).await).await;
        let code = param(&query(&url), "code").to_owned();
        let resp = self
            .token(&params(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("client_id", OUTSIDE_CLIENT),
                ("redirect_uri", OUTSIDE_REDIRECT),
                ("code_verifier", VERIFIER),
            ]))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let tokens: Value = resp.json().await.unwrap_or_default();
        string(&tokens, "access_token")
    }

    /// The status of the user's own `GET /v0/users/{user}` with `token` as its bearer.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn rest_user(&self, user: &TestUser, token: &str) -> StatusCode {
        let slug: &str = user.slug.as_ref();
        self.client
            .get(self.server.api_url(&format!("/v0/users/{slug}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(token),
            )
            .send()
            .await
            .expect("Request failed")
            .status()
    }

    /// The status of the server admin's `GET /v0/users` with `token` as its bearer.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn rest_admin(&self, token: &str) -> StatusCode {
        self.client
            .get(self.server.api_url("/v0/users"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(token),
            )
            .send()
            .await
            .expect("Request failed")
            .status()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn mcp(&self, token: &str) -> reqwest::Response {
        self.client
            .post(self.server.api_url("/mcp"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(token),
            )
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "list_organizations", "arguments": {}},
            }))
            .send()
            .await
            .expect("MCP request failed")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn set_admin(&self, user: &TestUser, admin: bool) {
        diesel::update(schema::user::table.filter(schema::user::uuid.eq(&user.uuid)))
            .set(schema::user::admin.eq(admin))
            .execute(&mut self.server.db_conn())
            .expect("Failed to set admin");
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn authorizations(&self) -> i64 {
        schema::oauth_authorization::table
            .count()
            .get_result(&mut self.server.db_conn())
            .expect("Failed to count authorizations")
    }
}

/// `plus.oauth` with the one outside app the tests connect.
fn oauth_config() -> Value {
    json!({
        "issuer": "https://api.example.com",
        "clients": [{
            "id": OUTSIDE_CLIENT,
            "metadata": {
                "client_id": OUTSIDE_CLIENT,
                "client_name": "Example Client",
                "redirect_uris": [OUTSIDE_REDIRECT],
                "grant_types": ["authorization_code", "refresh_token"],
            },
        }],
    })
}

fn cli_request() -> Vec<(&'static str, &'static str)> {
    vec![
        ("response_type", "code"),
        ("client_id", CLI),
        ("redirect_uri", LOOPBACK),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("state", STATE),
        ("device", DEVICE),
    ]
}

fn cli_request_with<'a>(changes: &[(&'a str, Option<&'a str>)]) -> Vec<(&'a str, &'a str)> {
    let mut request: Vec<(&'a str, &'a str)> = cli_request();
    for (name, value) in changes {
        request.retain(|(existing, _)| existing != name);
        if let Some(value) = value {
            request.push((name, value));
        }
    }
    request
}

fn refresh_params(refresh_token: &str) -> Vec<(String, String)> {
    params(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLI),
    ])
}

fn params(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn error_page(error: &str) -> Url {
    Url::parse(&format!("{CONSOLE_URL}/auth/authorize?error={error}")).expect("Invalid URL")
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
        Some(&*format!("{CONSOLE_URL}/auth/authorize")),
        "{location}"
    );
    let params = query(&location);
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

fn header<'r>(resp: &'r reqwest::Response, name: &str) -> &'r str {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn string(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .expect("Missing string")
        .to_owned()
}
