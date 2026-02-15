#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for OCI base endpoint (/v2/).

use bencher_api_tests::TestServer;
use bencher_json::RunnerUuid;
use http::StatusCode;

// GET /v2/ - OCI base endpoint (unauthenticated)
#[tokio::test]
async fn oci_base_unauthenticated() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .send()
        .await
        .expect("Request failed");

    // Should return 401 with WWW-Authenticate header
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET /v2/ - OCI base endpoint (authenticated)
#[tokio::test]
async fn oci_base_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("OCI User", "ocibase@example.com").await;
    let org = server.create_org(&user, "OCI Org").await;
    let project = server.create_project(&user, &org, "OCI Project").await;

    // Get an OCI token
    let oci_token = server.oci_pull_token(&user, &project);

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v2/ - OCI base endpoint (runner token rejected)
#[tokio::test]
async fn oci_base_runner_token_rejected() {
    let server = TestServer::new().await;
    let user = server.signup("OCI User", "ocirunner@example.com").await;
    let org = server.create_org(&user, "OCI Org").await;
    let project = server.create_project(&user, &org, "OCI Project").await;

    let runner_uuid: RunnerUuid = "00000000-0000-4000-8000-000000000001"
        .parse()
        .expect("valid UUID");
    let runner_token = server.oci_runner_pull_token(runner_uuid, project.slug.as_ref());

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .header("Authorization", format!("Bearer {}", runner_token))
        .send()
        .await
        .expect("Request failed");

    // Runner tokens should be rejected at the base endpoint (user-only)
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// OPTIONS /v2/ - CORS preflight
#[tokio::test]
async fn oci_base_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(reqwest::Method::OPTIONS, server.api_url("/v2/"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}
