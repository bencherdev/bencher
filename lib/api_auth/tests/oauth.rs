#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for OAuth endpoints (GitHub, Google).

use bencher_api_tests::TestServer;

// GET /v0/auth/github - OAuth redirect (requires plus feature)
#[tokio::test]
async fn github_oauth_redirect() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/github"))
        .send()
        .await
        .expect("Request failed");

    // Without GitHub OAuth configured, this returns an error
    // With plus feature and config, it would redirect (3xx)
    #[cfg(feature = "plus")]
    assert!(
        resp.status().is_client_error() || resp.status().is_redirection(),
        "Expected client error or redirect, got: {}",
        resp.status()
    );
    #[cfg(not(feature = "plus"))]
    assert!(resp.status().is_client_error());
}

// POST /v0/auth/github - OAuth callback without state
#[tokio::test]
async fn github_oauth_callback_no_state() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "code": "test-code"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/github"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Missing state should fail
    assert!(resp.status().is_client_error());
}

// GET /v0/auth/google - OAuth redirect (requires plus feature)
#[tokio::test]
async fn google_oauth_redirect() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/google"))
        .send()
        .await
        .expect("Request failed");

    // Without Google OAuth configured, this returns an error
    // With plus feature and config, it would redirect (3xx)
    #[cfg(feature = "plus")]
    assert!(
        resp.status().is_client_error() || resp.status().is_redirection(),
        "Expected client error or redirect, got: {}",
        resp.status()
    );
    #[cfg(not(feature = "plus"))]
    assert!(resp.status().is_client_error());
}

// POST /v0/auth/google - OAuth callback without state
#[tokio::test]
async fn google_oauth_callback_no_state() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "code": "test-code"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/google"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Missing state should fail
    assert!(resp.status().is_client_error());
}

// Verify OAuth endpoints exist (OPTIONS check would be here in production)
#[tokio::test]
async fn oauth_endpoints_exist() {
    let server = TestServer::new().await;

    // Just verify the endpoints respond (any response is fine)
    let github_resp = server
        .client
        .get(server.api_url("/v0/auth/github"))
        .send()
        .await
        .expect("GitHub request failed");

    let google_resp = server
        .client
        .get(server.api_url("/v0/auth/google"))
        .send()
        .await
        .expect("Google request failed");

    // Both should respond (even if with errors when not configured)
    assert!(github_resp.status().as_u16() > 0);
    assert!(google_resp.status().as_u16() > 0);
}

// Kills a sign-in with GitHub or Google whose state drops the app's request to connect, or
// carries one that does not wait for an answer.
#[cfg(feature = "plus")]
#[tokio::test]
async fn provider_state_returns_to_a_pending_authorization() {
    use bencher_api_tests::helpers::seed_oauth_authorization;
    use bencher_json::{DateTime, JsonOAuthUrl, Jwt, OAuthAuthorizationUuid};

    let server = provider_server().await;
    let pending = seed_oauth_authorization(&server, DateTime::now());

    for provider in ["github", "google"] {
        for (authorization, expected) in [
            (pending, Some(pending)),
            (OAuthAuthorizationUuid::new(), None),
        ] {
            let resp = server
                .client
                .get(server.api_url(&format!("/v0/auth/{provider}")))
                .query(&[("authorization", authorization.to_string())])
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), http::StatusCode::OK, "{provider}");
            let JsonOAuthUrl { url } = resp.json().await.expect("Failed to parse response");
            let state: Jwt = url
                .query_pairs()
                .find_map(|(name, value)| (name == "state").then(|| value.into_owned()))
                .expect("No state")
                .parse()
                .expect("Invalid state");
            let claims = server
                .token_key()
                .validate_oauth(&state)
                .expect("Invalid state");
            assert_eq!(claims.state.authorization, expected, "{provider}");
        }
    }
}

// Kills a sign-in start with GitHub or Google that a cache may keep.
#[cfg(feature = "plus")]
#[tokio::test]
async fn provider_starts_are_never_cached() {
    let server = provider_server().await;
    for provider in ["github", "google"] {
        let resp = server
            .client
            .get(server.api_url(&format!("/v0/auth/{provider}")))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), http::StatusCode::OK, "{provider}");
        assert_eq!(
            resp.headers()[http::header::CACHE_CONTROL],
            "no-store",
            "{provider}"
        );
    }
}

#[cfg(feature = "plus")]
async fn provider_server() -> TestServer {
    use bencher_api_tests::{
        TestServerOptions,
        helpers::{github_client, google_client, oauth_server},
    };

    let server = TestServer::new_with_options(TestServerOptions {
        oauth: Some(oauth_server()),
        github_client: Some(github_client()),
        google_client: Some(google_client()),
        ..TestServerOptions::default()
    })
    .await;
    // Signing in with a provider takes an Enterprise license on a self-hosted server.
    let user = server.signup("License User", "license@example.com").await;
    let org = server.create_org(&user, "Licensed").await;
    server.license_org(&user, &org).await;
    server
}

// GitHub and Google sign-in on a Self-Hosted server are on while one organization on the server
// holds a valid license, its grace included, and off with none or with only an expired one.
#[cfg(feature = "plus")]
#[tokio::test]
async fn provider_sign_in_needs_a_valid_license_on_the_server() {
    use bencher_api_tests::{
        TestServerOptions,
        helpers::{github_client, google_client},
        license::{DAY, TEST_LICENSE_SERIES, after, sign_license},
    };
    use http::StatusCode;

    let server = TestServer::new_with_options(TestServerOptions {
        github_client: Some(github_client()),
        google_client: Some(google_client()),
        ..TestServerOptions::default()
    })
    .await;
    let statuses = async || {
        let mut statuses = Vec::new();
        for path in ["/v0/auth/github", "/v0/auth/google"] {
            let resp = server
                .client
                .get(server.api_url(path))
                .send()
                .await
                .expect("Request failed");
            statuses.push(resp.status());
        }
        statuses
    };
    let refused = vec![StatusCode::PAYMENT_REQUIRED; 2];
    assert_eq!(statuses().await, refused, "no license");

    let user = server.signup("License User", "license@example.com").await;
    let now = server.now();
    let expired = server.create_org(&user, "Expired").await;
    server.store_license(
        &expired,
        &sign_license(
            expired.uuid,
            TEST_LICENSE_SERIES,
            after(now, -400 * DAY),
            after(now, -31 * DAY),
        ),
    );
    assert_eq!(statuses().await, refused, "only an expired license");

    let in_grace = server.create_org(&user, "In Grace").await;
    server
        .license_org_with(
            &user,
            &in_grace,
            &sign_license(
                in_grace.uuid,
                TEST_LICENSE_SERIES,
                after(now, -40 * DAY),
                after(now, -DAY),
            ),
        )
        .await;
    assert_eq!(
        statuses().await,
        vec![StatusCode::OK; 2],
        "a license in its grace"
    );
}
