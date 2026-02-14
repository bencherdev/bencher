#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for server root endpoint.

use bencher_api_tests::TestServer;

// GET / - root path is accessible
#[tokio::test]
async fn root_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/"))
        .send()
        .await
        .expect("Request failed");

    // Root returns 200 OK or redirect
    assert!(resp.status().is_success() || resp.status().is_redirection());
}

// POST / - only available with plus feature
#[tokio::test]
async fn root_post() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/"))
        .send()
        .await
        .expect("Request failed");

    // Root POST behavior depends on plus feature
    // With plus: returns 404 NOT_FOUND (endpoint exists but needs proper request)
    // Without plus: returns client error
    assert!(
        resp.status().is_client_error(),
        "Expected client error, got: {}",
        resp.status()
    );
}

// GET / - with auth header still works
#[tokio::test]
async fn root_get_with_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "rootauth@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert!(resp.status().is_success() || resp.status().is_redirection());
}
