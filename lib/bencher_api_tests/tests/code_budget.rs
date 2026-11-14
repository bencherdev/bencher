#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The budget of wrong codes for an address, which holds on every server: in a build without
//! `plus`, which has no rate limiter and no Turnstile, as in one with it. Run it without `plus` as
//! `cargo nextest run -p bencher_api_tests --no-default-features`, since every API crate's own
//! tests build the API with `plus`.

use bencher_api_tests::{LogCapture, TestServer};
use bencher_json::SignInCode;
use bencher_schema::model::user::session::cookie::PendingCookie;
use reqwest::{
    StatusCode,
    header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
};

const EMAIL: &str = "ada@example.com";

// Ten wrong codes for an address, across its sign-ins, pause its codes, the right one included.
// Kills a budget that holds only in a build with `plus`.
#[tokio::test]
async fn ten_wrong_codes_pause_the_address_in_every_build() {
    let log = LogCapture::default();
    let server = TestServer::new_with_log(log.logger()).await;

    for _ in 0..2 {
        let pending = start(&server).await;
        let wrong = wrong_code(&log.sign_in().0);
        for _ in 0..5 {
            assert_eq!(
                redeem(&server, &pending, &wrong).await.0,
                StatusCode::FORBIDDEN,
                "a wrong code"
            );
        }
    }

    let pending = start(&server).await;
    let (code, _link) = log.sign_in();
    let (status, body) = redeem(&server, &pending, &code).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("codes_paused"), "{body}");
}

/// Start a sign-in for [`EMAIL`] from the console, and return the browser's pending cookie.
#[expect(clippy::expect_used, reason = "test helper")]
async fn start(server: &TestServer) -> String {
    let resp = server
        .client
        .post(server.api_url("/v0/auth/email"))
        .header("Sec-Fetch-Site", "same-origin")
        .header(CONTENT_TYPE, "application/json")
        .body(format!(r#"{{"email":"{EMAIL}"}}"#))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "Sign-in not started");
    let name = pending_cookie_name(server);
    resp.headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| {
            cookie
                .split(';')
                .next()?
                .strip_prefix(&name)?
                .strip_prefix('=')
                .map(ToOwned::to_owned)
        })
        .expect("No pending cookie")
}

/// Send `code` from the browser that holds `pending`, and return the API's status and body.
#[expect(clippy::expect_used, reason = "test helper")]
async fn redeem(server: &TestServer, pending: &str, code: &SignInCode) -> (StatusCode, String) {
    let resp = server
        .client
        .post(server.api_url("/v0/auth/code"))
        .header(COOKIE, format!("{}={pending}", pending_cookie_name(server)))
        .header("Sec-Fetch-Site", "same-origin")
        .header(CONTENT_TYPE, "application/json")
        .body(format!(r#"{{"code":"{}"}}"#, code.as_ref()))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read the body"))
}

fn pending_cookie_name(server: &TestServer) -> String {
    PendingCookie::new(&server.context().console_url)
        .name()
        .to_owned()
}

#[expect(clippy::expect_used, reason = "test helper")]
fn wrong_code(code: &SignInCode) -> SignInCode {
    let wrong = if code.as_ref() == "000000" {
        "000001"
    } else {
        "000000"
    };
    wrong.parse().expect("Invalid code")
}
