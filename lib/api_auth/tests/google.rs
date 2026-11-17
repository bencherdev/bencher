#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in with Google.

use bencher_api_tests::{
    TestOrg, TestServer, TestServerOptions, TestUser,
    github::GitHubServer,
    helpers::{google_client, user_organizations},
};
use bencher_json::{JsonOAuthUrl, JsonOAuthUser};

const CODE: &str = "code-marker";

fn exchanged() -> (u16, String) {
    (
        200,
        r#"{"access_token": "user-token-marker", "token_type": "bearer", "expires_in": 3600}"#
            .to_owned(),
    )
}

fn user_info(email: &str) -> (u16, String) {
    (
        200,
        format!(r#"{{"name": "Muriel Bagge", "email": "{email}", "email_verified": true}}"#),
    )
}

async fn add_sso(server: &TestServer, admin: &TestUser, org: &TestOrg, domain: &str) {
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{org_slug}/sso")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&serde_json::json!({ "domain": domain }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    assert!(
        status.is_success(),
        "{status}: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// Sign in with Google as the account the stand-in names, and return the user.
async fn sign_in(server: &TestServer) -> JsonOAuthUser {
    let response = server
        .client
        .get(server.api_url("/v0/auth/google"))
        .send()
        .await
        .expect("Request failed");
    let authorize: JsonOAuthUrl = response.json().await.expect("Failed to parse the URL");
    let state = authorize
        .url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("No state");
    let response = server
        .client
        .post(server.api_url("/v0/auth/google"))
        .json(&serde_json::json!({ "code": CODE, "state": state }))
        .send()
        .await
        .expect("Request failed");
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap_or_default()
    );
    response.json().await.expect("Failed to parse the user")
}

// Fails if a Google sign-in at the SSO domain of an organization with a valid license does not
// join the user to it, as an email confirmation does.
#[tokio::test]
async fn google_sign_in_joins_an_enterprise_sso_organization() {
    let google = GitHubServer::start(vec![exchanged(), user_info("user@google.example.com")]).await;
    let server = TestServer::new_with_options(TestServerOptions {
        google_client: Some(google_client().with_base_url(&google.base())),
        ..TestServerOptions::default()
    })
    .await;
    let admin = server.signup("Admin User", "googleadmin@example.com").await;
    let org = server.create_org(&admin, "Google Sso Org").await;
    server.license_org(&admin, &org).await;
    add_sso(&server, &admin, &org, "google.example.com").await;

    let user = sign_in(&server).await.user.user;

    assert!(user_organizations(&server, user.uuid).contains(&org.uuid));
    google.received().await;
}
