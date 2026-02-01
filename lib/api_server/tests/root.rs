#![allow(unused_crate_dependencies, clippy::tests_outside_test_module, clippy::redundant_test_prefix, clippy::uninlined_format_args)]
//! Integration tests for server root endpoint.

use bencher_api_tests::TestServer;

// GET / - root path is accessible
#[tokio::test]
async fn test_root_get() {
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
async fn test_root_post() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/"))
        .send()
        .await
        .expect("Request failed");

    // Root POST behavior depends on plus feature
    #[cfg(feature = "plus")]
    assert!(resp.status().is_success() || resp.status().is_redirection());
    #[cfg(not(feature = "plus"))]
    assert!(resp.status().is_client_error());
}

// GET / - with auth header still works
#[tokio::test]
async fn test_root_get_with_auth() {
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
