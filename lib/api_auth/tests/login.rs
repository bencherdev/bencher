#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration test file"
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
        #[cfg(feature = "plus")]
        authorization: None,
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
async fn login_success() {
    let server = TestServer::new().await;
    create_user(&server, "login@example.com").await;

    let body = JsonLogin {
        email: "login@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
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
async fn login_unknown_user() {
    let server = TestServer::new().await;

    let body = JsonLogin {
        email: "unknown@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
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
async fn login_invalid_email() {
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
async fn login_empty_body() {
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

// Kills a login whose emailed link drops the app's request to connect, or carries one that no
// longer waits for an answer: unknown, expired, or already answered. Only the email that returns
// to one warns of it.
#[cfg(feature = "plus")]
#[tokio::test]
async fn login_link_returns_to_a_pending_authorization() {
    use bencher_api_tests::{
        LogCapture, TestServerOptions,
        helpers::{oauth_server, seed_oauth_authorization},
    };
    use bencher_json::{DateTime, JsonOAuthDecision, OAuthAuthorizationUuid};

    let capture = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        oauth: Some(oauth_server()),
        log: Some(capture.logger()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server.signup("Connect User", "connect@example.com").await;
    let now = DateTime::now();

    let pending = seed_oauth_authorization(&server, now);
    login(&server, "connect@example.com", Some(pending)).await;
    let link = capture.email_link("Confirm Login");
    assert_eq!(link.origin(), server.context().console_url.origin());
    assert_eq!(link.path(), "/auth/confirm");
    assert_eq!(
        query_names(&link),
        ["token", "authorization"],
        "Link: {link}"
    );
    assert_eq!(
        query_value(&link, "authorization"),
        Some(pending.to_string())
    );
    assert!(
        capture.email("Confirm Login").contains(CONNECT_NOTICE),
        "The email does not warn of the app"
    );

    let answered = seed_oauth_authorization(&server, now);
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/oauth/authorizations/{answered}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&JsonOAuthDecision { approve: true })
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    // Seeded last, since seeding drops every expired request.
    let expired = seed_oauth_authorization(
        &server,
        DateTime::try_from(now.timestamp() - 30 * 60).unwrap(),
    );
    for authorization in [OAuthAuthorizationUuid::new(), expired, answered] {
        login(&server, "connect@example.com", Some(authorization)).await;
        let link = capture.email_link("Confirm Login");
        assert_eq!(query_names(&link), ["token"], "Link: {link}");
        assert!(
            !capture.email("Confirm Login").contains(CONNECT_NOTICE),
            "The email warns of an app it does not return to"
        );
    }
}

// Kills a server without OAuth configured that still sends sign-ins back to a request to connect.
#[cfg(feature = "plus")]
#[tokio::test]
async fn login_link_ignores_authorization_without_oauth() {
    use bencher_api_tests::{LogCapture, TestServerOptions, helpers::seed_oauth_authorization};
    use bencher_json::DateTime;

    let capture = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        log: Some(capture.logger()),
        ..TestServerOptions::default()
    })
    .await;
    create_user(&server, "dormant@example.com").await;
    let pending = seed_oauth_authorization(&server, DateTime::now());

    login(&server, "dormant@example.com", Some(pending)).await;
    let link = capture.email_link("Confirm Login");
    assert_eq!(query_names(&link), ["token"], "Link: {link}");
}

#[cfg(feature = "plus")]
async fn login(
    server: &TestServer,
    email: &str,
    authorization: Option<bencher_json::OAuthAuthorizationUuid>,
) {
    let body = JsonLogin {
        email: email.parse().unwrap(),
        plan: None,
        invite: None,
        recaptcha_token: None,
        authorization,
    };
    let resp = server
        .client
        .post(server.api_url("/v0/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "Login failed");
}

#[cfg(feature = "plus")]
fn query_names(url: &url::Url) -> Vec<String> {
    url.query_pairs()
        .map(|(name, _)| name.into_owned())
        .collect()
}

#[cfg(feature = "plus")]
fn query_value(url: &url::Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

/// The line a sign-in email that returns to an app's request to connect carries.
#[cfg(feature = "plus")]
const CONNECT_NOTICE: &str = "An app asked to connect to your Bencher account. If you did not start this, ignore this email.";
