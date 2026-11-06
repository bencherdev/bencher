#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in with GitHub.

use bencher_api_tests::{
    LogCapture, TestServer,
    github::{GitHubServer, Received},
    session_cookie,
};
use bencher_json::{Clock, JsonOAuthUrl, JsonSignedIn, PlanLevel, UserSlug};
use http::{
    StatusCode,
    header::{COOKIE, SET_COOKIE, USER_AGENT},
};

const REDIRECT_URL: &str = "http://localhost:3000/auth/github";
const CODE: &str = "code-marker";

fn exchanged() -> (u16, String) {
    (
        200,
        r#"{"access_token": "user-token-marker", "token_type": "bearer", "scope": ""}"#.to_owned(),
    )
}

fn signed_in_user(login: &str) -> (u16, String) {
    let profile = format!("https://github.example/{login}");
    (
        200,
        format!(
            r#"{{"login": "{login}", "id": 1, "node_id": "U_1", "avatar_url": "{profile}", "gravatar_id": "", "url": "{profile}", "html_url": "{profile}", "followers_url": "{profile}", "following_url": "{profile}", "gists_url": "{profile}", "starred_url": "{profile}", "subscriptions_url": "{profile}", "organizations_url": "{profile}", "repos_url": "{profile}", "events_url": "{profile}", "received_events_url": "{profile}", "type": "User", "site_admin": false}}"#
        ),
    )
}

fn primary_email(email: &str) -> (u16, String) {
    (
        200,
        format!(
            r#"[{{"email": "{email}", "primary": true, "verified": true, "visibility": null}}]"#
        ),
    )
}

// Sign-in names its own page, which leaves the app's first callback URL free for another page.
// Kills an authorize URL or an exchange that drops the redirect or names any other.
#[tokio::test]
async fn sign_in_names_its_redirect_on_authorize_and_exchange() {
    let github = GitHubServer::start(octo_cat()).await;
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
        .header("Sec-Fetch-Site", "same-origin")
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

// Kills a sign-up that refuses a login an account provider issues, such as the `handle_shortcode`
// login of a GitHub Enterprise Managed User, and one that cannot name the new user from it.
#[tokio::test]
async fn sign_up_takes_a_login_with_an_underscore() {
    let login = "muriel_bagge";
    let github = GitHubServer::start(vec![
        exchanged(),
        signed_in_user(login),
        primary_email("muriel_bagge@example.com"),
    ])
    .await;
    let server =
        TestServer::new_with_github(github.base(), Clock::System, LogCapture::default().logger())
            .await;
    let admin = server.signup("Admin User", "admin@example.com").await;
    let org = server.create_org(&admin, "Licensed Org").await;
    server
        .license_org(&admin, &org, PlanLevel::Enterprise)
        .await;

    let state = start(&server).await;
    let response = server
        .client
        .post(server.api_url("/v0/auth/github"))
        .header("Sec-Fetch-Site", "same-origin")
        .json(&serde_json::json!({ "code": CODE, "state": state }))
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap_or_default()
    );
    let signed_in: JsonSignedIn = response.json().await.expect("Failed to parse the user");
    let slug: UserSlug = "muriel-bagge".parse().expect("Invalid slug");
    assert_eq!(signed_in.user.name.as_ref(), login);
    assert_eq!(signed_in.user.slug, slug);
}

// Until providers move to a flow of their own, the console's old exchange signs in with a session:
// it begins one in its cookie, in place of the one the browser presented, and answers with no token.
// Kills an exchange from another origin, one checked only after GitHub is asked, a token in the
// answer, a presented session left behind, and a session recorded as anything but GitHub's.
#[tokio::test]
async fn provider_sign_in_begins_a_session_in_its_cookie() {
    let github = GitHubServer::start(octo_cat()).await;
    let server =
        TestServer::new_with_github(github.base(), Clock::System, LogCapture::default().logger())
            .await;
    let admin = server.signup("Admin User", "admin@example.com").await;
    let org = server.create_org(&admin, "Licensed Org").await;
    server
        .license_org(&admin, &org, PlanLevel::Enterprise)
        .await;
    let presented = session_cookie(&server.session(&admin));
    let state = start(&server).await;
    let exchange = |site: Option<&str>| {
        let mut request = server
            .client
            .post(server.api_url("/v0/auth/github"))
            .header(COOKIE, &presented)
            .header(USER_AGENT, FIREFOX)
            .json(&serde_json::json!({ "code": CODE, "state": state }));
        if let Some(site) = site {
            request = request.header("Sec-Fetch-Site", site);
        }
        request.send()
    };

    for site in [None, Some("cross-site"), Some("same-site")] {
        let response = exchange(site).await.expect("Request failed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{site:?}");
    }
    let response = exchange(Some("same-origin")).await.expect("Request failed");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    // GitHub's script holds one exchange, which only the console's own request reached.
    assert_eq!(github.received().await.len(), 3);
    let set_cookie = response
        .headers()
        .get(SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("No session cookie")
        .to_owned();
    let (cookie, attributes) = set_cookie.split_once("; ").expect("No cookie attributes");
    assert!(
        cookie.starts_with("__Host-bencher_session="),
        "{set_cookie}"
    );
    assert!(
        attributes.starts_with("HttpOnly; Secure; SameSite=Strict; Path=/"),
        "{set_cookie}"
    );
    let body: serde_json::Value = response.json().await.expect("Invalid answer");
    assert_eq!(body.get("token"), None, "{body}");
    assert_eq!(body["user"]["email"], "octo-cat@example.com");
    assert_eq!(body["new"], true);

    let session = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(COOKIE, cookie)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(session.status(), StatusCode::OK);
    let session: serde_json::Value = session.json().await.expect("Invalid session");
    assert_eq!(session["method"], "github");
    assert_eq!(session["user"]["email"], "octo-cat@example.com");

    let ended = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(COOKIE, &presented)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(ended.status(), StatusCode::UNAUTHORIZED);
}

const FIREFOX: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";

/// The state a GitHub sign-in starts with.
async fn start(server: &TestServer) -> String {
    let response = server
        .client
        .get(server.api_url("/v0/auth/github"))
        .send()
        .await
        .expect("Request failed");
    let authorize: JsonOAuthUrl = response.json().await.expect("Failed to parse the URL");
    authorize
        .url
        .query_pairs()
        .find_map(|(k, v)| (k == "state").then(|| v.into_owned()))
        .expect("No state")
}

/// GitHub's answers to one exchange for the user `octo-cat`.
fn octo_cat() -> Vec<(u16, String)> {
    vec![
        exchanged(),
        signed_in_user("octo-cat"),
        primary_email("octo-cat@example.com"),
    ]
}
