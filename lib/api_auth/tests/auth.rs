//! Integration tests for auth endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonAuthAck, JsonLogin, JsonSignup};
use http::StatusCode;

// POST /v0/auth/signup - public
#[tokio::test]
async fn test_auth_signup() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "test@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let ack: JsonAuthAck = resp.json().await.expect("Failed to parse response");
    assert_eq!(ack.email, body.email);
}

// POST /v0/auth/signup - duplicate email returns conflict
#[tokio::test]
async fn test_auth_signup_duplicate_email() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "duplicate@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
    };

    // First signup
    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Second signup with same email returns conflict
    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// POST /v0/auth/signup - must agree to terms
#[tokio::test]
async fn test_auth_signup_must_agree() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "noagree@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: false,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/auth/login - public
#[tokio::test]
async fn test_auth_login() {
    let server = TestServer::new().await;

    // First signup
    let signup = JsonSignup {
        name: "Login User".parse().unwrap(),
        slug: None,
        email: "login@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
    };

    server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&signup)
        .send()
        .await
        .expect("Signup failed");

    // Then try login
    let body = JsonLogin {
        email: "login@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Login sends email, returns 202 Accepted
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/auth/login - unknown user returns not found
#[tokio::test]
async fn test_auth_login_unknown_user() {
    let server = TestServer::new().await;

    let body = JsonLogin {
        email: "unknown@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Unknown user returns not found
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/auth/confirm - requires valid token
#[tokio::test]
async fn test_auth_confirm_invalid_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "token": "invalid-token"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/confirm"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invalid token should fail
    assert!(resp.status().is_client_error());
}

// POST /v0/auth/accept - requires valid invite token
#[tokio::test]
async fn test_auth_accept_invalid_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "invite": "invalid-token"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invalid token should fail
    assert!(resp.status().is_client_error());
}

// GET /v0/auth/github - OAuth redirect (plus feature)
#[tokio::test]
async fn test_auth_github_redirect() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/github"))
        .send()
        .await
        .expect("Request failed");

    // Without GitHub OAuth configured, this returns an error
    // With plus feature and config, it would redirect
    assert!(resp.status().is_client_error() || resp.status().is_redirection());
}

// GET /v0/auth/google - OAuth redirect (plus feature)
#[tokio::test]
async fn test_auth_google_redirect() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/google"))
        .send()
        .await
        .expect("Request failed");

    // Without Google OAuth configured, this returns an error
    // With plus feature and config, it would redirect
    assert!(resp.status().is_client_error() || resp.status().is_redirection());
}
