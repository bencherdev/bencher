#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the OCI token endpoint (GET and POST /v0/auth/oci/token).

use bencher_api_tests::TestServer;
use bencher_json::ProjectKey;
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

// GET with multiple scope params (Docker 29 containerd pattern) → 200 with merged actions
#[tokio::test]
async fn oci_token_multiple_scopes() {
    let server = TestServer::new().await;
    let user = server
        .signup("MultiScope User", "multiscope@example.com")
        .await;
    let org = server.create_org(&user, "MultiScope Org").await;
    let project = server
        .create_project(&user, &org, "MultiScope Project")
        .await;

    let api_key = server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX)
        .expect("Failed to create API key");

    let project_slug: &str = project.slug.as_ref();
    let url = format!(
        "/v0/auth/oci/token?scope=repository:{project_slug}:pull&scope=repository:{project_slug}:pull,push&service=localhost"
    );

    let resp = server
        .client
        .get(server.api_url(&url))
        .basic_auth(user.email.as_ref(), Some(api_key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["token"].is_string());
}

// GET with multiple scope params using project key auth
#[tokio::test]
async fn oci_token_multiple_scopes_project_key() {
    let server = TestServer::new().await;
    let user = server
        .signup("MultiScopeKey User", "multiscopekey@example.com")
        .await;
    let org = server.create_org(&user, "MultiScopeKey Org").await;
    let project = server
        .create_project(&user, &org, "MultiScopeKey Project")
        .await;
    let project_key = server
        .create_project_key(&user, &project, "multi-scope-key")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let url = format!(
        "/v0/auth/oci/token?scope=repository:{project_slug}:pull&scope=repository:{project_slug}:pull,push&service=localhost"
    );

    let resp = server
        .client
        .get(server.api_url(&url))
        .basic_auth(project_slug, Some(project_key.key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["token"].is_string());
}

// POST with grant_type=password and user credentials → 200 with access_token
#[tokio::test]
async fn oci_token_post_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("PostAuth User", "postauth@example.com").await;
    let org = server.create_org(&user, "PostAuth Org").await;
    let project = server.create_project(&user, &org, "PostAuth Project").await;

    let api_key = server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX)
        .expect("Failed to create API key");

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:push");

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", user.email.as_ref()),
            ("password", api_key.as_ref()),
            ("scope", &scope),
            ("service", "localhost"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(
        body["access_token"].is_string(),
        "POST response should contain access_token"
    );
    assert!(
        body["token"].is_null(),
        "POST response should NOT contain token"
    );
    assert!(body["expires_in"].is_number());
    assert!(body["issued_at"].is_string());

    let token = body["access_token"].as_str().unwrap();
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

// POST with grant_type=password and project key → 200 with access_token
#[tokio::test]
async fn oci_token_post_project_key() {
    let server = TestServer::new().await;
    let user = server.signup("PostKey User", "postkey@example.com").await;
    let org = server.create_org(&user, "PostKey Org").await;
    let project = server.create_project(&user, &org, "PostKey Project").await;
    let project_key = server.create_project_key(&user, &project, "post-key").await;

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:push");

    assert!(
        project_key.key.as_ref().starts_with(ProjectKey::PREFIX),
        "Project key should start with bencher_run_ prefix"
    );

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", project_slug),
            ("password", project_key.key.as_ref()),
            ("scope", &scope),
            ("service", "localhost"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["access_token"].is_string());

    let token = body["access_token"].as_str().unwrap();
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

// POST with wrong credentials → 401
#[tokio::test]
async fn oci_token_post_bad_credentials() {
    let server = TestServer::new().await;
    let user = server.signup("PostBad User", "postbad@example.com").await;

    let bad_api_key = server
        .token_key()
        .new_api_key("wrong@example.com".parse().unwrap(), u32::MAX)
        .expect("Failed to create API key");

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", user.email.as_ref()),
            ("password", bad_api_key.as_ref()),
            ("scope", "repository:test:push"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// POST with grant_type=refresh_token → 400
#[tokio::test]
async fn oci_token_post_refresh_token_rejected() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", "some-token"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST with missing grant_type → 400
#[tokio::test]
async fn oci_token_post_missing_grant_type() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[("scope", "repository:test:push")])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST with space-separated scopes (containerd joins scopes with spaces in POST body)
#[tokio::test]
async fn oci_token_post_space_separated_scopes() {
    let server = TestServer::new().await;
    let user = server
        .signup("PostScopes User", "postscopes@example.com")
        .await;
    let org = server.create_org(&user, "PostScopes Org").await;
    let project = server
        .create_project(&user, &org, "PostScopes Project")
        .await;
    let project_key = server
        .create_project_key(&user, &project, "post-scopes-key")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:pull repository:{project_slug}:pull,push");

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", project_slug),
            ("password", project_key.key.as_ref()),
            ("scope", &scope),
            ("service", "localhost"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await.expect("Invalid JSON");
    assert!(body["access_token"].is_string());
}

// POST with grant_type=password but no credentials → 400
#[tokio::test]
async fn oci_token_post_no_credentials() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("scope", "repository:test:push"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
