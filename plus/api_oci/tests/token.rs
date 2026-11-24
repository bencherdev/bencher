#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the OCI token endpoint (GET and POST /v0/auth/oci/token).

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use bencher_json::{JsonUserKeyCreated, ProjectKey, UserKey};
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

// --- User API key (`bencher_user_*`) credentials ------------------------------

#[expect(
    clippy::expect_used,
    clippy::missing_assert_message,
    reason = "test helper outside #[test] fns"
)]
async fn mint_user_key(server: &TestServer, user_slug: &str, token: &str) -> JsonUserKeyCreated {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{user_slug}/keys")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&serde_json::json!({"name": "oci-login-key"}))
        .send()
        .await
        .expect("Failed to create user key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Failed to parse new user key")
}

// GET with email + user API key → 200, token works for push, same as email + JWT
#[tokio::test]
async fn oci_token_user_key() {
    let server = TestServer::new().await;
    let user = server
        .signup("UserKey User", "userkey-oci@example.com")
        .await;
    let org = server.create_org(&user, "UserKey Org").await;
    let project = server.create_project(&user, &org, "UserKey Project").await;
    let user_slug: &str = user.slug.as_ref();
    let created = mint_user_key(&server, user_slug, &user.token).await;

    assert!(
        created.key.as_ref().starts_with(UserKey::PREFIX),
        "User key should start with bencher_user_ prefix"
    );

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:push");

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/auth/oci/token?scope={scope}&service=localhost"
        )))
        .basic_auth(user.email.as_ref(), Some(created.key.as_ref()))
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

// POST with grant_type=password and email + user API key → 200 with access_token
#[tokio::test]
async fn oci_token_post_user_key() {
    let server = TestServer::new().await;
    let user = server
        .signup("PostUserKey User", "postuserkey-oci@example.com")
        .await;
    let org = server.create_org(&user, "PostUserKey Org").await;
    let project = server
        .create_project(&user, &org, "PostUserKey Project")
        .await;
    let user_slug: &str = user.slug.as_ref();
    let created = mint_user_key(&server, user_slug, &user.token).await;

    let project_slug: &str = project.slug.as_ref();
    let scope = format!("repository:{project_slug}:push");

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", user.email.as_ref()),
            ("password", created.key.as_ref()),
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

// GET with an email that does not own the user key → 401
#[tokio::test]
async fn oci_token_user_key_wrong_email() {
    let server = TestServer::new().await;
    let user = server
        .signup("KeyOwner User", "keyowner-oci@example.com")
        .await;
    let other = server
        .signup("OtherUser User", "otheruser-oci@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();
    let created = mint_user_key(&server, user_slug, &user.token).await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token?scope=repository:test:push"))
        .basic_auth(other.email.as_ref(), Some(created.key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET with a revoked user key → 401
#[tokio::test]
async fn oci_token_user_key_revoked() {
    let server = TestServer::new().await;
    let user = server
        .signup("RevokedKey User", "revokedkey-oci@example.com")
        .await;
    let user_slug: &str = user.slug.as_ref();
    let created = mint_user_key(&server, user_slug, &user.token).await;

    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{user_slug}/keys/{}", created.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Failed to revoke user key");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token?scope=repository:test:push"))
        .basic_auth(user.email.as_ref(), Some(created.key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET with a well-formed but unknown user key → 401
#[tokio::test]
async fn oci_token_user_key_unknown() {
    let server = TestServer::new().await;
    let user = server
        .signup("UnknownKey User", "unknownkey-oci@example.com")
        .await;

    let unknown_key = format!("{}{}", UserKey::PREFIX, "A".repeat(30));
    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token?scope=repository:test:push"))
        .basic_auth(user.email.as_ref(), Some(&unknown_key))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET with a `bencher_user_`-prefixed password that is not a valid key → 401
#[tokio::test]
async fn oci_token_user_key_malformed() {
    let server = TestServer::new().await;
    let user = server
        .signup("MalformedKey User", "malformedkey-oci@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/auth/oci/token?scope=repository:test:push"))
        .basic_auth(user.email.as_ref(), Some("bencher_user_not-a-real-key"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// --- Scopes for more than one repository ---------------------------------------
//
// When a layer may be mounted from another repository, containerd asks for the
// mount source's pull scope beside the push scope, and it sorts the scopes. So a
// mount source that sorts first comes before the repository being pushed.

// GET with a user API token, mount source first → a push token for the pushed repository
#[tokio::test]
async fn oci_token_mount_source_first_user() {
    let server = TestServer::new().await;
    let user = server
        .signup("MountFirst User", "mountfirst@example.com")
        .await;
    let org = server.create_org(&user, "MountFirst Org").await;
    let project_a = server.create_project(&user, &org, "MountFirst A").await;
    let project_b = server.create_project(&user, &org, "MountFirst B").await;

    let api_key = server
        .token_key()
        .new_api_key(user.email.clone(), u32::MAX)
        .expect("Failed to create API key");

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let url = format!(
        "/v0/auth/oci/token?scope=repository:{slug_a}:pull&scope=repository:{slug_b}:pull,push&service=localhost"
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
    let token = body["token"].as_str().expect("Missing token");

    let blob_data = b"mount source first blob";
    let push_token_a = server.oci_push_token(&user, &project_a);
    assert_eq!(
        push_blob(&server, slug_a, &push_token_a, blob_data).await,
        StatusCode::CREATED
    );

    // The token is for the pushed repository only, so a mount from the source falls back to an upload.
    let mount_resp = server
        .client
        .post(server.api_url(&format!(
            "/v2/{slug_b}/blobs/uploads?digest={}&from={slug_a}",
            compute_digest(blob_data)
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Mount request failed");
    assert_eq!(mount_resp.status(), StatusCode::ACCEPTED);

    assert_eq!(
        push_blob(&server, slug_b, token, blob_data).await,
        StatusCode::CREATED
    );
}

// POST with the project key of the pushed repository, mount source first → a push token for it
#[tokio::test]
async fn oci_token_post_mount_source_first_project_key() {
    let server = TestServer::new().await;
    let user = server
        .signup("MountFirstKey User", "mountfirstkey@example.com")
        .await;
    let org = server.create_org(&user, "MountFirstKey Org").await;
    let project_a = server.create_project(&user, &org, "MountFirstKey A").await;
    let project_b = server.create_project(&user, &org, "MountFirstKey B").await;
    let project_key_b = server
        .create_project_key(&user, &project_b, "mount-first-key-b")
        .await;

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let scope = format!("repository:{slug_a}:pull repository:{slug_b}:pull,push");

    let resp = server
        .client
        .post(server.api_url("/v0/auth/oci/token"))
        .form(&[
            ("grant_type", "password"),
            ("username", slug_b),
            ("password", project_key_b.key.as_ref()),
            ("scope", &scope),
            ("service", "localhost"),
        ])
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("Invalid JSON");
    let token = body["access_token"].as_str().expect("Missing access_token");

    assert_eq!(
        push_blob(&server, slug_b, token, b"mount source first key blob").await,
        StatusCode::CREATED
    );
}

// GET with the project key of the mount source, push scope for another project → 403
#[tokio::test]
async fn oci_token_mount_source_project_key_cannot_push_other() {
    let server = TestServer::new().await;
    let user = server
        .signup("MountSourceKey User", "mountsourcekey@example.com")
        .await;
    let org = server.create_org(&user, "MountSourceKey Org").await;
    let project_a = server.create_project(&user, &org, "MountSourceKey A").await;
    let project_b = server.create_project(&user, &org, "MountSourceKey B").await;
    let project_key_a = server
        .create_project_key(&user, &project_a, "mount-source-key-a")
        .await;

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let url = format!(
        "/v0/auth/oci/token?scope=repository:{slug_a}:pull&scope=repository:{slug_b}:pull,push&service=localhost"
    );

    let resp = server
        .client
        .get(server.api_url(&url))
        .basic_auth(slug_a, Some(project_key_a.key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read body");
    assert!(
        body.contains("Project key is not authorized for the requested repository"),
        "Unexpected body: {body}"
    );
}

// GET with a user API token, push scope for a claimed project the user cannot push to → 403
#[tokio::test]
async fn oci_token_mount_source_first_user_without_push_permission() {
    let server = TestServer::new().await;
    let owner = server
        .signup("MountOwner User", "mountowner@example.com")
        .await;
    let org = server.create_org(&owner, "MountOwner Org").await;
    let project_a = server.create_project(&owner, &org, "MountOwner A").await;
    let project_b = server.create_project(&owner, &org, "MountOwner B").await;
    let outsider = server
        .signup("MountOutsider User", "mountoutsider@example.com")
        .await;

    let api_key = server
        .token_key()
        .new_api_key(outsider.email.clone(), u32::MAX)
        .expect("Failed to create API key");

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let url = format!(
        "/v0/auth/oci/token?scope=repository:{slug_a}:pull&scope=repository:{slug_b}:pull,push&service=localhost"
    );

    let resp = server
        .client
        .get(server.api_url(&url))
        .basic_auth(outsider.email.as_ref(), Some(api_key.as_ref()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read body");
    assert!(
        body.contains(&format!("Access denied to repository: {slug_b}")),
        "Unexpected body: {body}"
    );
}

#[expect(clippy::expect_used, reason = "test helper outside #[test] fns")]
async fn push_blob(server: &TestServer, repository: &str, token: &str, data: &[u8]) -> StatusCode {
    server
        .client
        .put(server.api_url(&format!(
            "/v2/{repository}/blobs/uploads?digest={}",
            compute_digest(data)
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .expect("Blob push failed")
        .status()
}
