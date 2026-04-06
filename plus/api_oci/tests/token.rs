#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::indexing_slicing
)]
//! Integration tests for the OCI token endpoint (GET /v0/auth/oci/token).

use bencher_api_tests::TestServer;
use http::StatusCode;
use serde_json::Value;

// Anonymous token request (no Basic auth) → returns a valid public OCI token
#[tokio::test]
async fn oci_token_anonymous() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(
            server
                .api_url("/v0/auth/oci/token?scope=repository:some-project:push&service=localhost"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["token"].is_string(), "Response should contain a token");
    assert!(
        body["expires_in"].is_number(),
        "Response should contain expires_in"
    );
    assert!(
        body["issued_at"].is_string(),
        "Response should contain issued_at"
    );

    // The token should be accepted by the /v2/ base endpoint
    let token = body["token"].as_str().unwrap();
    let v2_resp = server
        .client
        .get(server.api_url("/v2/"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(v2_resp.status(), StatusCode::OK);
}

// Anonymous token request without scope → returns a valid token (base endpoint only)
#[tokio::test]
async fn oci_token_anonymous_no_scope() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["token"].is_string());
}

// Authenticated token request with valid Basic auth (email:api_key) → returns an auth OCI token
#[tokio::test]
async fn oci_token_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Token User", "tokenuser@example.com").await;
    let org = server.create_org(&user, "Token Org").await;
    let project = server.create_project(&user, &org, "Token Project").await;

    // The OCI token endpoint expects an API key (aud: "api_key"), not a client token
    let api_key = server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX)
        .expect("Failed to create API key");

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:push");

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/auth/oci/token?scope={scope}&service=localhost"
        )))
        .basic_auth(user.email.as_ref(), Some(api_key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    let token = body["token"].as_str().unwrap();

    // The auth token should be accepted by the /v2/ base endpoint
    let v2_resp = server
        .client
        .get(server.api_url("/v2/"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(v2_resp.status(), StatusCode::OK);
}

// Authenticated token request with wrong password → 401
#[tokio::test]
async fn oci_token_bad_credentials() {
    let server = TestServer::new().await;
    let user = server.signup("BadCred User", "badcred@example.com").await;

    // Use the email but a garbage password — not a valid JWT so extract_basic_auth
    // will fail to parse it and fall through to anonymous token issuance.
    // To test actual credential rejection, we need a structurally valid JWT
    // that fails validation.
    let bad_api_key = server
        .token_key()
        .new_api_key("wrong@example.com".parse().unwrap(), u32::MAX)
        .expect("Failed to create API key");

    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token?scope=repository:test:push"))
        .basic_auth(user.email.as_ref(), Some(bad_api_key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}
