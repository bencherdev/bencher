#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in with GitHub.

use bencher_api_tests::{
    LogCapture, TestServer,
    github::{GitHubServer, Received},
};
use bencher_json::{Clock, JsonOAuthUrl, PlanLevel};

const REDIRECT_URL: &str = "http://localhost:3000/auth/github";
const CODE: &str = "code-marker";

// Sign-in names its own page, which leaves the app's first callback URL free for another page.
// Kills an authorize URL or an exchange that drops the redirect or names any other.
#[tokio::test]
async fn sign_in_names_its_redirect_on_authorize_and_exchange() {
    let profile = "https://github.example/octo-cat";
    let github = GitHubServer::start(vec![
        (
            200,
            r#"{"access_token": "user-token-marker", "token_type": "bearer", "scope": ""}"#
                .to_owned(),
        ),
        (
            200,
            format!(
                r#"{{"login": "octo-cat", "id": 1, "node_id": "U_1", "avatar_url": "{profile}", "gravatar_id": "", "url": "{profile}", "html_url": "{profile}", "followers_url": "{profile}", "following_url": "{profile}", "gists_url": "{profile}", "starred_url": "{profile}", "subscriptions_url": "{profile}", "organizations_url": "{profile}", "repos_url": "{profile}", "events_url": "{profile}", "received_events_url": "{profile}", "type": "User", "site_admin": false}}"#
            ),
        ),
        (
            200,
            r#"[{"email": "octo-cat@example.com", "primary": true, "verified": true, "visibility": null}]"#
                .to_owned(),
        ),
    ])
    .await;
    let server =
        TestServer::new_with_github(github.base(), Clock::System, LogCapture::default().logger())
            .await;
    // A server that is not Bencher Cloud allows GitHub sign-in only with a license.
    let admin = server.signup("Admin User", "admin@example.com").await;
    let org = server.create_org(&admin, "Licensed Org").await;
    server
        .license_org(&admin, &org, PlanLevel::Enterprise)
        .await;

    let response = server
        .client
        .get(server.api_url("/v0/auth/github"))
        .send()
        .await
        .expect("Request failed");
    let authorize: JsonOAuthUrl = response.json().await.expect("Failed to parse the URL");
    let query = |key: &str| {
        authorize
            .url
            .query_pairs()
            .find_map(|(k, v)| (k == key).then(|| v.into_owned()))
            .expect("No such query pair")
    };
    let response = server
        .client
        .post(server.api_url("/v0/auth/github"))
        .json(&serde_json::json!({ "code": CODE, "state": query("state") }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(query("redirect_uri"), REDIRECT_URL);
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap_or_default()
    );
    let [exchange, _user, _emails]: [Received; 3] = github
        .received()
        .await
        .try_into()
        .ok()
        .expect("Wrong request count");
    let form = exchange.form();
    assert_eq!(form.get("code").map(String::as_str), Some(CODE));
    assert_eq!(
        form.get("redirect_uri").map(String::as_str),
        Some(REDIRECT_URL)
    );
}
