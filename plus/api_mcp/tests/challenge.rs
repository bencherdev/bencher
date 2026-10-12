#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the apps domain verification challenge
//! (GET /.well-known/openai-apps-challenge).

use bencher_api_tests::TestServer;
use http::{Method, StatusCode};

const CHALLENGE_PATH: &str = "/.well-known/openai-apps-challenge";

// The configured token is the whole body, as plain text, on whatever host the request names
#[tokio::test]
async fn openai_apps_challenge() {
    const CHALLENGE: &str = "openai-apps-challenge-0123456789abcdef";
    let server = TestServer::new_with_openai_apps_challenge(
        CHALLENGE.parse().expect("Invalid challenge token"),
    )
    .await;

    let resp = server
        .client
        .get(server.api_url(CHALLENGE_PATH))
        .header(reqwest::header::HOST, "mcp.bencher.dev")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(reqwest::header::CONTENT_TYPE),
        Some(&reqwest::header::HeaderValue::from_static(
            "text/plain; charset=utf-8"
        ))
    );
    assert_eq!(
        resp.headers()
            .get(reqwest::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&reqwest::header::HeaderValue::from_static("*"))
    );
    assert_eq!(
        resp.bytes().await.expect("Failed to read body"),
        CHALLENGE.as_bytes()
    );

    let resp = server
        .client
        .request(Method::OPTIONS, server.api_url(CHALLENGE_PATH))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    for (name, value) in [
        (reqwest::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        (reqwest::header::ACCESS_CONTROL_ALLOW_METHODS, "GET"),
    ] {
        assert_eq!(
            resp.headers().get(&name),
            Some(&reqwest::header::HeaderValue::from_static(value)),
            "{name}"
        );
    }
}

#[tokio::test]
async fn openai_apps_challenge_not_configured() {
    let server = TestServer::new().await;

    for method in [Method::GET, Method::OPTIONS] {
        let resp = server
            .client
            .request(method.clone(), server.api_url(CHALLENGE_PATH))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method}");
    }
}
