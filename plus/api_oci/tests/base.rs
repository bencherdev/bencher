#![cfg(feature = "plus")]
#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for OCI base endpoint (/v2/).

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::{compute_digest, create_oci_manifest};
use bencher_json::RunnerUuid;
use bencher_token::OciAction;
use http::StatusCode;

// GET /v2/ - no token → 401
#[tokio::test]
async fn oci_base_unauthenticated() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET /v2/ - public OCI token → 200
#[tokio::test]
async fn oci_base_public_token() {
    let server = TestServer::new().await;

    let public_token = server.oci_public_token("any-project", &[OciAction::Push]);

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v2/ - auth OCI token → 200
#[tokio::test]
async fn oci_base_auth_token() {
    let server = TestServer::new().await;
    let user = server.signup("OCI User", "ocibase@example.com").await;
    let org = server.create_org(&user, "OCI Org").await;
    let project = server.create_project(&user, &org, "OCI Project").await;

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

// GET /v2/ - runner OCI token → 401 (runners don't use /v2/)
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// GET /v2/ - invalid JWT → 401
#[tokio::test]
async fn oci_base_invalid_token_rejected() {
    let server = TestServer::new().await;

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

// Smoke test: full push flow using public OCI token to an unclaimed project
#[tokio::test]
async fn oci_base_public_push_smoke() {
    let server = TestServer::new().await;
    let slug = "smoke-unauth-project";

    // Get a public OCI token (simulates what Docker gets from token endpoint without Basic auth)
    let public_token = server.oci_public_token(slug, &[OciAction::Pull, OciAction::Push]);

    // 1. GET /v2/ — version check with public token
    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Upload config blob (public token)
    let config_data = b"config data";
    let config_digest = compute_digest(config_data);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/blobs/uploads?digest={config_digest}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(config_data.to_vec())
        .send()
        .await
        .expect("Config upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2b. HEAD blob check (public token) — Docker does this during push
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{slug}/blobs/{config_digest}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("HEAD config blob failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Upload layer blob (public token)
    let layer_data = b"layer data";
    let layer_digest = compute_digest(layer_data);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/blobs/uploads?digest={layer_digest}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(layer_data.to_vec())
        .send()
        .await
        .expect("Layer upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3b. HEAD blob check (public token)
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{slug}/blobs/{layer_digest}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("HEAD layer blob failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Upload manifest (public token)
    let manifest = create_oci_manifest(&config_digest, &layer_digest);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/manifests/latest")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Manifest upload failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("docker-content-digest"));
}
