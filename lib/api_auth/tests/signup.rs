#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for auth signup endpoint.

use bencher_api_tests::TestServer;
use bencher_json::{JsonSignup, system::auth::JsonAuthAck};
use http::StatusCode;

// POST /v0/auth/signup - successful signup
#[tokio::test]
async fn test_signup_success() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "signup@example.com".parse().unwrap(),
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

// POST /v0/auth/signup - custom slug
#[tokio::test]
async fn test_signup_with_custom_slug() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Custom Slug User".parse().unwrap(),
        slug: Some("custom-slug-user".parse().unwrap()),
        email: "customslug@example.com".parse().unwrap(),
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
}

// POST /v0/auth/signup - duplicate email returns conflict
#[tokio::test]
async fn test_signup_duplicate_email() {
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
async fn test_signup_must_agree_to_terms() {
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

// POST /v0/auth/signup - invalid email format
#[tokio::test]
async fn test_signup_invalid_email() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "name": "Test User",
        "email": "not-an-email",
        "i_agree": true
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
