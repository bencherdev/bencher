#![cfg(feature = "plus")]
#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for OCI base endpoint (/v2/).

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::{compute_digest, create_oci_manifest};
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

    // Should return 200 OK without authentication
    assert_eq!(resp.status(), StatusCode::OK);
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v2/ - OCI base endpoint (runner token rejected — only user tokens accepted)
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&runner_token),
        )
        .send()
        .await
        .expect("Request failed");

    // A token was provided but is not a valid user token — hard 401
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// GET /v2/ - OCI base endpoint (expired/wrong-key user token rejected)
#[tokio::test]
async fn oci_base_invalid_claims_rejected() {
    let server = TestServer::new().await;

    // Use a structurally valid JWT that will fail claim validation
    // (signed with a different key / wrong claims)
    let bad_jwt = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.\
                   eyJhdWQiOiJvY2kiLCJleHAiOjEsImlhdCI6MCwiaXNzIjoiaHR0cDovL2xvY2FsaG9zdDozMDAwLyIsInN1YiI6ImJhZEBiYWQuY29tIiwib2NpIjp7InJlcG9zaXRvcnkiOm51bGwsImFjdGlvbnMiOltdfX0.\
                   AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bad_jwt),
        )
        .send()
        .await
        .expect("Request failed");

    // A structurally valid JWT was provided but fails validation — hard 401
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
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

// Smoke test: full unauthenticated push flow to an unclaimed project
#[tokio::test]
async fn oci_base_unauthenticated_push_smoke() {
    let server = TestServer::new().await;
    let slug = "smoke-unauth-project";

    // 1. GET /v2/ — version check (no auth)
    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Upload config blob (no auth)
    let config_data = b"config data";
    let config_digest = compute_digest(config_data);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/blobs/uploads?digest={config_digest}")))
        .header("Content-Type", "application/octet-stream")
        .body(config_data.to_vec())
        .send()
        .await
        .expect("Config upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2b. HEAD blob check (no auth) — Docker does this during push
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{slug}/blobs/{config_digest}")))
        .send()
        .await
        .expect("HEAD config blob failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Upload layer blob (no auth)
    let layer_data = b"layer data";
    let layer_digest = compute_digest(layer_data);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/blobs/uploads?digest={layer_digest}")))
        .header("Content-Type", "application/octet-stream")
        .body(layer_data.to_vec())
        .send()
        .await
        .expect("Layer upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3b. HEAD blob check (no auth)
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{slug}/blobs/{layer_digest}")))
        .send()
        .await
        .expect("HEAD layer blob failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Upload manifest (no auth)
    let manifest = create_oci_manifest(&config_digest, &layer_digest);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/manifests/latest")))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Manifest upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("docker-content-digest"));
}
