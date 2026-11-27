#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in with Google.

use bencher_api_tests::{
    TestServer,
    github::{GitHubServer as ScriptedServer, Received},
    session_cookie,
};
use bencher_json::{JsonOAuthUrl, PlanLevel};
use http::{
    StatusCode,
    header::{COOKIE, SET_COOKIE},
};

const CODE: &str = "code-marker";

// Until providers move to a flow of their own, the console's old exchange signs in with a session:
// it begins one in its cookie, in place of the one the browser presented, and answers with no token.
// Kills a Google exchange from another origin, one checked only after Google is asked, a token in
// the answer, a presented session left behind, and a session recorded as anything but Google's.
#[tokio::test]
async fn google_sign_in_begins_a_session_in_its_cookie() {
    let google = ScriptedServer::start(vec![
        (
            200,
            r#"{"access_token": "user-token-marker", "token_type": "bearer"}"#.to_owned(),
        ),
        (
            200,
            r#"{"name": "Ada Lovelace", "email": "ada@example.com", "email_verified": true}"#
                .to_owned(),
        ),
    ])
    .await;
    let server = TestServer::new_with_google(&google.base()).await;
    // A server that is not Bencher Cloud allows Google sign-in only with a license.
    let admin = server.signup("Admin User", "admin@example.com").await;
    let org = server.create_org(&admin, "Licensed Org").await;
    server
        .license_org(&admin, &org, PlanLevel::Enterprise)
        .await;
    let presented = session_cookie(&server.session(&admin));
    let authorize: JsonOAuthUrl = server
        .client
        .get(server.api_url("/v0/auth/google"))
        .send()
        .await
        .expect("Request failed")
        .json()
        .await
        .expect("Failed to parse the URL");
    let state = authorize
        .url
        .query_pairs()
        .find_map(|(k, v)| (k == "state").then(|| v.into_owned()))
        .expect("No state");
    let exchange = |site: Option<&str>| {
        let mut request = server
            .client
            .post(server.api_url("/v0/auth/google"))
            .header(COOKIE, &presented)
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
    // Google's script holds one exchange, which only the console's own request reached.
    let [token, user_info]: [Received; 2] = google
        .received()
        .await
        .try_into()
        .ok()
        .expect("Wrong request count");
    assert_eq!(token.form().get("code").map(String::as_str), Some(CODE));
    assert!(
        user_info.request_line().starts_with("GET /v1/userinfo "),
        "{}",
        user_info.request_line()
    );
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
    assert_eq!(body["user"]["email"], "ada@example.com");
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
    assert_eq!(session["method"], "google");
    assert_eq!(session["user"]["email"], "ada@example.com");

    let ended = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(COOKIE, &presented)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(ended.status(), StatusCode::UNAUTHORIZED);
}
