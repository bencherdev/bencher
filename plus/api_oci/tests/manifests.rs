#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix
)]
//! Integration tests for OCI manifest endpoints.

use bencher_api_tests::TestServer;
use http::StatusCode;
use sha2::{Digest as _, Sha256};

/// Helper to compute SHA256 digest in OCI format
fn compute_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    format!("sha256:{}", hex::encode(hash))
}

/// Create a minimal OCI manifest JSON for testing
fn create_test_manifest(config_digest: &str, layer_digest: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest,
            "size": 100
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                "digest": layer_digest,
                "size": 200
            }
        ]
    })
    .to_string()
}

// PUT /v2/{name}/manifests/{reference} - Upload manifest with tag
#[tokio::test]
async fn test_manifest_put_with_tag() {
    let server = TestServer::new().await;
    let user = server
        .signup("Manifest User", "manifestput@example.com")
        .await;
    let org = server.create_org(&user, "Manifest Org").await;
    let project = server.create_project(&user, &org, "Manifest Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // First upload config and layer blobs
    let config_data = b"config data";
    let layer_data = b"layer data";
    let config_digest = compute_digest(config_data);
    let layer_digest = compute_digest(layer_data);

    // Upload config blob
    server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, config_digest
        )))
        .header("Authorization", format!("Bearer {}", oci_token))
        .body(config_data.to_vec())
        .send()
        .await
        .expect("Config upload failed");

    // Upload layer blob
    server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, layer_digest
        )))
        .header("Authorization", format!("Bearer {}", oci_token))
        .body(layer_data.to_vec())
        .send()
        .await
        .expect("Layer upload failed");

    // Upload manifest with tag
    let manifest = create_test_manifest(&config_digest, &layer_digest);
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// PUT /v2/{name}/manifests/{reference} - Upload manifest with digest reference
#[tokio::test]
async fn test_manifest_put_by_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("DigestManifest User", "manifestdigest@example.com")
        .await;
    let org = server.create_org(&user, "DigestManifest Org").await;
    let project = server
        .create_project(&user, &org, "DigestManifest Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let config_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let layer_digest = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    let manifest = create_test_manifest(config_digest, layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, manifest_digest
        )))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// HEAD /v2/{name}/manifests/{reference} - Check manifest exists
#[tokio::test]
async fn test_manifest_exists() {
    let server = TestServer::new().await;
    let user = server
        .signup("ExistsManifest User", "manifestexists@example.com")
        .await;
    let org = server.create_org(&user, "ExistsManifest Org").await;
    let project = server
        .create_project(&user, &org, "ExistsManifest Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    let layer_digest = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
    let manifest = create_test_manifest(config_digest, layer_digest);

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/v1.0.0", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Check if it exists
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/v1.0.0", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("docker-content-digest"));
    assert!(resp.headers().contains_key("content-type"));
}

// HEAD /v2/{name}/manifests/{reference} - Manifest not found
#[tokio::test]
async fn test_manifest_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("NotFoundManifest User", "manifestnotfound@example.com")
        .await;
    let org = server.create_org(&user, "NotFoundManifest Org").await;
    let project = server
        .create_project(&user, &org, "NotFoundManifest Project")
        .await;

    let oci_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/nonexistent", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v2/{name}/manifests/{reference} - Download manifest by tag
#[tokio::test]
async fn test_manifest_get_by_tag() {
    let server = TestServer::new().await;
    let user = server
        .signup("GetManifest User", "manifestget@example.com")
        .await;
    let org = server.create_org(&user, "GetManifest Org").await;
    let project = server
        .create_project(&user, &org, "GetManifest Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:5555555555555555555555555555555555555555555555555555555555555555";
    let layer_digest = "sha256:6666666666666666666666666666666666666666666666666666666666666666";
    let manifest = create_test_manifest(config_digest, layer_digest);

    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/stable", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest.clone())
        .send()
        .await
        .expect("Upload failed");

    // Download it
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/stable", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.expect("Failed to read body");
    // Parse both to compare (formatting might differ)
    let uploaded: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(uploaded, downloaded);
}

// GET /v2/{name}/manifests/{reference} - Download manifest by digest
#[tokio::test]
async fn test_manifest_get_by_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("DigestGet User", "manifestdigestget@example.com")
        .await;
    let org = server.create_org(&user, "DigestGet Org").await;
    let project = server
        .create_project(&user, &org, "DigestGet Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:7777777777777777777777777777777777777777777777777777777777777777";
    let layer_digest = "sha256:8888888888888888888888888888888888888888888888888888888888888888";
    let manifest = create_test_manifest(config_digest, layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());

    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/test", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest.clone())
        .send()
        .await
        .expect("Upload failed");

    // Download by digest
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, manifest_digest
        )))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// DELETE /v2/{name}/manifests/{reference} - Delete manifest by tag
#[tokio::test]
async fn test_manifest_delete_by_tag() {
    let server = TestServer::new().await;
    let user = server
        .signup("DeleteTag User", "manifestdeletetag@example.com")
        .await;
    let org = server.create_org(&user, "DeleteTag Org").await;
    let project = server
        .create_project(&user, &org, "DeleteTag Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:9999999999999999999999999999999999999999999999999999999999999999";
    let layer_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let manifest = create_test_manifest(config_digest, layer_digest);

    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/to-delete", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Upload failed");

    // Delete by tag
    let resp = server
        .client
        .delete(server.api_url(&format!("/v2/{}/manifests/to-delete", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify tag is gone
    let pull_token = server.oci_pull_token(&user, &project);
    let check_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/to-delete", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(check_resp.status(), StatusCode::NOT_FOUND);
}

// DELETE /v2/{name}/manifests/{reference} - Delete manifest by digest
#[tokio::test]
async fn test_manifest_delete_by_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("DeleteDigest User", "manifestdeletedigest@example.com")
        .await;
    let org = server.create_org(&user, "DeleteDigest Org").await;
    let project = server
        .create_project(&user, &org, "DeleteDigest Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let layer_digest = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    let manifest = create_test_manifest(config_digest, layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());

    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/digest-delete", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Upload failed");

    // Delete by digest
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, manifest_digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// OPTIONS /v2/{name}/manifests/{reference} - CORS preflight
#[tokio::test]
async fn test_manifest_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/manifests/latest"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}
