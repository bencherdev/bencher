//! Integration tests for auth confirm endpoint.

use bencher_api_tests::TestServer;
use bencher_json::{JsonConfirm, JsonSignup, system::auth::JsonAuthUser};
use http::StatusCode;

// POST /v0/auth/confirm - invalid token format
#[tokio::test]
async fn test_confirm_invalid_token_format() {
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

// POST /v0/auth/confirm - empty token
#[tokio::test]
async fn test_confirm_empty_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "token": ""
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/confirm"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert!(resp.status().is_client_error());
}

// POST /v0/auth/confirm - successful confirmation with valid token
#[tokio::test]
async fn test_confirm_success() {
    let server = TestServer::new().await;

    // First signup
    let signup_body = JsonSignup {
        name: "Confirm User".parse().unwrap(),
        slug: None,
        email: "confirm@example.com".parse().unwrap(),
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
        .json(&signup_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Generate auth token for confirmation
    let auth_token = server
        .token_key()
        .new_auth("confirm@example.com".parse().unwrap(), u32::MAX)
        .expect("Failed to generate token");

    let confirm_body = JsonConfirm { token: auth_token };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/confirm"))
        .json(&confirm_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let auth_user: JsonAuthUser = resp.json().await.expect("Failed to parse response");
    assert_eq!(auth_user.user.email.as_ref(), "confirm@example.com");
}

// POST /v0/auth/confirm - token for non-existent user
#[tokio::test]
async fn test_confirm_nonexistent_user() {
    let server = TestServer::new().await;

    // Generate auth token for a user that doesn't exist
    let auth_token = server
        .token_key()
        .new_auth("nonexistent@example.com".parse().unwrap(), u32::MAX)
        .expect("Failed to generate token");

    let confirm_body = JsonConfirm { token: auth_token };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/confirm"))
        .json(&confirm_body)
        .send()
        .await
        .expect("Request failed");

    // User not found
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
