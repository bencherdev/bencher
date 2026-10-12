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
    use bencher_api_tests::{
        TestServerOptions,
        helpers::{github_client, google_client, oauth_server, seed_oauth_authorization},
    };
    use bencher_json::{DateTime, JsonOAuthUrl, Jwt, OAuthAuthorizationUuid, PlanLevel};

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
    server.license_org(&user, &org, PlanLevel::Enterprise).await;
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
