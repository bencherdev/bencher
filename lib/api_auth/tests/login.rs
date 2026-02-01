#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Integration tests for auth login endpoint.

use bencher_api_tests::TestServer;
use bencher_json::{JsonLogin, JsonSignup, system::auth::JsonAuthAck};
use http::StatusCode;

// Helper to create a user for login tests
async fn create_user(server: &TestServer, email: &str) {
    let body = JsonSignup {
        name: "Login User".parse().unwrap(),
        slug: None,
        email: email.parse().unwrap(),
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
        .json(&body)
        .send()
        .await
        .expect("Signup failed");
}

// POST /v0/auth/login - successful login
#[tokio::test]
async fn test_login_success() {
    let server = TestServer::new().await;
    create_user(&server, "login@example.com").await;

    let body = JsonLogin {
        email: "login@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
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
    let ack: JsonAuthAck = resp.json().await.expect("Failed to parse response");
    assert_eq!(ack.email.as_ref(), "login@example.com");
}

// POST /v0/auth/login - unknown user returns not found
#[tokio::test]
async fn test_login_unknown_user() {
    let server = TestServer::new().await;

    let body = JsonLogin {
        email: "unknown@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/auth/login - invalid email format
#[tokio::test]
async fn test_login_invalid_email() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "email": "not-an-email"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/auth/login - empty body
#[tokio::test]
async fn test_login_empty_body() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
