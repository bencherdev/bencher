#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix,
    clippy::indexing_slicing
)]
//! Integration tests for OCI manifest endpoints.

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use bencher_token::OciAction;
use http::StatusCode;

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

// =============================================================================
// Tests for validate_push_access branches (manifest endpoints)
// =============================================================================

// Manifest upload to UNCLAIMED project (unauthenticated) should succeed
#[tokio::test]
async fn test_manifest_put_unclaimed() {
    let server = TestServer::new().await;

    let config_digest = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let layer_digest = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let manifest = create_test_manifest(config_digest, layer_digest);

    // Push manifest to a new project slug (will auto-create unclaimed project)
    let resp = server
        .client
        .put(server.api_url("/v2/unclaimed-manifest-project/manifests/latest"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// Manifest upload to CLAIMED project without auth should fail with 401
#[tokio::test]
async fn test_manifest_put_unauthenticated_to_claimed() {
    let server = TestServer::new().await;

    // Create a claimed project
    let user = server
        .signup(
            "Manifest Claimed User",
            "manifestclaimedproject@example.com",
        )
        .await;
    let org = server.create_org(&user, "Manifest Claimed Org").await;
    let project = server
        .create_project(&user, &org, "Manifest Claimed Project")
        .await;

    let project_slug: &str = project.slug.as_ref();

    let config_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let layer_digest = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    let manifest = create_test_manifest(config_digest, layer_digest);

    // Try to push without authentication - should fail
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// Authenticated manifest upload to UNCLAIMED project should auto-claim
#[tokio::test]
async fn test_manifest_put_authenticated_to_unclaimed() {
    let server = TestServer::new().await;

    // First, create an unclaimed project by pushing without authentication
    let unclaimed_slug = "unclaimed-manifest-to-claim";

    let config_digest1 = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let layer_digest1 = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let manifest1 = create_test_manifest(config_digest1, layer_digest1);

    let create_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/v1", unclaimed_slug)))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest1)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::CREATED);

    // Now create a user and push with authentication - should auto-claim
    let user = server
        .signup("Manifest Claimer", "manifestclaimer@example.com")
        .await;

    let oci_token = server.oci_token(&user, unclaimed_slug, &[OciAction::Push]);

    let config_digest2 = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    let layer_digest2 = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let manifest2 = create_test_manifest(config_digest2, layer_digest2);

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/v2", unclaimed_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest2)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify the project is now claimed by trying unauthenticated push again
    let config_digest3 = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let layer_digest3 = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let manifest3 = create_test_manifest(config_digest3, layer_digest3);

    let unauth_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/v3", unclaimed_slug)))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest3)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);
}

// Manifest upload to non-existent project by UUID should return 404
#[tokio::test]
async fn test_manifest_put_nonexistent_uuid() {
    let server = TestServer::new().await;
    let user = server
        .signup("Manifest UUID User", "manifestuuidpush@example.com")
        .await;

    // Use a random UUID that doesn't exist
    let fake_uuid = "00000000-0000-0000-0000-000000000000";

    let oci_token = server.oci_token(&user, fake_uuid, &[OciAction::Push]);

    let config_digest = "sha256:1234567890123456789012345678901234567890123456789012345678901234";
    let layer_digest = "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
    let manifest = create_test_manifest(config_digest, layer_digest);

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", fake_uuid)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Authenticated manifest upload to non-existent slug should auto-create
#[tokio::test]
async fn test_manifest_put_nonexistent_slug_authenticated() {
    let server = TestServer::new().await;
    let user = server
        .signup("Manifest Slug User", "manifestslugpush@example.com")
        .await;

    // Use a new slug that doesn't exist yet
    let new_slug = "auto-created-manifest-project";

    let oci_token = server.oci_token(&user, new_slug, &[OciAction::Push]);

    let config_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let layer_digest = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    let manifest = create_test_manifest(config_digest, layer_digest);

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", new_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);

    // The project should now exist and be claimed by the user
    // Verify by trying unauthenticated push - should be rejected
    let config_digest2 = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    let layer_digest2 = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
    let manifest2 = create_test_manifest(config_digest2, layer_digest2);

    let unauth_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/v2", new_slug)))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest2)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);
}

// Manifest with an unsupported media type should be rejected
#[tokio::test]
async fn test_manifest_put_invalid_media_type() {
    let server = TestServer::new().await;
    let user = server
        .signup("MediaType User", "mediatype@example.com")
        .await;
    let org = server.create_org(&user, "MediaType Org").await;
    let project = server
        .create_project(&user, &org, "MediaType Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // A manifest with an unrecognized mediaType in the body
    let manifest = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.evil.custom.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "size": 100
        },
        "layers": []
    })
    .to_string();

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.evil.custom.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Unsupported manifest media type should be rejected"
    );
}

// PUT /v2/{name}/manifests/{tag} - Tag overwrite should succeed and update the tag
#[tokio::test]
async fn test_manifest_tag_overwrite() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagOverwrite User", "manifesttagoverwrite@example.com")
        .await;
    let org = server.create_org(&user, "TagOverwrite Org").await;
    let project = server
        .create_project(&user, &org, "TagOverwrite Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload first manifest as "latest"
    let manifest1 = create_test_manifest(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    );
    let digest1 = compute_digest(manifest1.as_bytes());

    let resp1 = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest1)
        .send()
        .await
        .expect("Upload 1 failed");
    assert_eq!(resp1.status(), StatusCode::CREATED);

    // Upload second manifest as "latest" (overwrite)
    let manifest2 = create_test_manifest(
        "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "sha256:4444444444444444444444444444444444444444444444444444444444444444",
    );
    let digest2 = compute_digest(manifest2.as_bytes());

    let resp2 = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest2)
        .send()
        .await
        .expect("Upload 2 failed");
    assert_eq!(resp2.status(), StatusCode::CREATED);

    // Verify "latest" now resolves to digest2, not digest1
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/latest", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("HEAD failed");
    assert_eq!(resp.status(), StatusCode::OK);

    let returned_digest = resp
        .headers()
        .get("docker-content-digest")
        .expect("Missing digest header")
        .to_str()
        .expect("Invalid digest header");
    assert_eq!(
        returned_digest, digest2,
        "Tag should point to the new manifest"
    );
    assert_ne!(
        returned_digest, digest1,
        "Tag should no longer point to the old manifest"
    );
}

// HEAD and GET should return the correct content-type header matching the manifest media type
#[tokio::test]
async fn test_manifest_content_type_round_trip() {
    let server = TestServer::new().await;
    let user = server
        .signup("ContentType User", "manifestcontenttype@example.com")
        .await;
    let org = server.create_org(&user, "ContentType Org").await;
    let project = server
        .create_project(&user, &org, "ContentType Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a manifest
    let config_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let layer_digest = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let manifest = create_test_manifest(config_digest, layer_digest);

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/ct-test", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    let pull_token = server.oci_pull_token(&user, &project);

    // HEAD should return the correct content-type
    let head_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/ct-test", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("HEAD request failed");
    assert_eq!(head_resp.status(), StatusCode::OK);
    let head_ct = head_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type on HEAD")
        .to_str()
        .expect("Invalid content-type header");
    assert_eq!(
        head_ct, "application/vnd.oci.image.manifest.v1+json",
        "HEAD content-type should match manifest media type"
    );

    // GET should return the correct content-type
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/ct-test", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET request failed");
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_ct = get_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type on GET")
        .to_str()
        .expect("Invalid content-type header");
    assert_eq!(
        get_ct, "application/vnd.oci.image.manifest.v1+json",
        "GET content-type should match manifest media type"
    );
}

// GET /v2/{name}/manifests/{digest} - Non-existent digest should return 404
#[tokio::test]
async fn test_manifest_get_nonexistent_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("ManifestNotFound User", "manifestnotfound@example.com")
        .await;
    let org = server.create_org(&user, "ManifestNotFound Org").await;
    let project = server
        .create_project(&user, &org, "ManifestNotFound Project")
        .await;

    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let fake_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, fake_digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Docker Manifest V2 Tests
// =============================================================================

/// Create a Docker V2 manifest JSON for testing
fn create_docker_v2_manifest(config_digest: &str, layer_digest: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {
            "mediaType": "application/vnd.docker.container.image.v1+json",
            "digest": config_digest,
            "size": 100
        },
        "layers": [
            {
                "mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip",
                "digest": layer_digest,
                "size": 200
            }
        ]
    })
    .to_string()
}

// PUT Docker V2 manifest with correct Content-Type, verify 201, GET back and verify content-type round-trip
#[tokio::test]
async fn test_manifest_put_docker_v2() {
    let server = TestServer::new().await;
    let user = server
        .signup("DockerV2Put User", "dockerv2put@example.com")
        .await;
    let org = server.create_org(&user, "DockerV2Put Org").await;
    let project = server
        .create_project(&user, &org, "DockerV2Put Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let config_digest = "sha256:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
    let layer_digest = "sha256:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
    let manifest = create_docker_v2_manifest(config_digest, layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());

    // PUT with Docker V2 Content-Type
    let put_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/docker-v2-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .body(manifest.clone())
        .send()
        .await
        .expect("PUT failed");

    assert_eq!(put_resp.status(), StatusCode::CREATED);
    assert!(put_resp.headers().contains_key("location"));
    assert!(put_resp.headers().contains_key("docker-content-digest"));

    // GET back and verify content-type round-trip
    let get_resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, manifest_digest
        )))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");

    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_ct = get_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type on GET")
        .to_str()
        .expect("Invalid content-type header");
    assert_eq!(
        get_ct, "application/vnd.docker.distribution.manifest.v2+json",
        "GET content-type should match Docker V2 media type"
    );

    let body = get_resp.text().await.expect("Failed to read body");
    let uploaded: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(uploaded, downloaded);
}

// Tag-based retrieval of a Docker V2 manifest
#[tokio::test]
async fn test_manifest_get_docker_v2_by_tag() {
    let server = TestServer::new().await;
    let user = server
        .signup("DockerV2Tag User", "dockerv2tag@example.com")
        .await;
    let org = server.create_org(&user, "DockerV2Tag Org").await;
    let project = server
        .create_project(&user, &org, "DockerV2Tag Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let config_digest = "sha256:c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
    let layer_digest = "sha256:d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4";
    let manifest = create_docker_v2_manifest(config_digest, layer_digest);

    // Upload with tag
    let put_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/docker-v2-get-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .body(manifest.clone())
        .send()
        .await
        .expect("PUT failed");
    assert_eq!(put_resp.status(), StatusCode::CREATED);

    // Retrieve by tag
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/docker-v2-get-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");

    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_ct = get_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type")
        .to_str()
        .expect("Invalid content-type");
    assert_eq!(
        get_ct, "application/vnd.docker.distribution.manifest.v2+json",
        "Tag-based GET should return Docker V2 content-type"
    );

    let body = get_resp.text().await.expect("Failed to read body");
    let uploaded: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(uploaded, downloaded);
}

// =============================================================================
// Docker Manifest List Tests
// =============================================================================

/// Create a Docker manifest list JSON for testing
fn create_docker_manifest_list() -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.list.v2+json",
        "manifests": [
            {
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "digest": "sha256:e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5",
                "size": 528,
                "platform": {
                    "architecture": "amd64",
                    "os": "linux"
                }
            },
            {
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "digest": "sha256:f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6",
                "size": 528,
                "platform": {
                    "architecture": "arm64",
                    "os": "linux"
                }
            }
        ]
    })
    .to_string()
}

// PUT Docker manifest list with platform entries, verify round-trip
#[tokio::test]
async fn test_manifest_put_docker_manifest_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("DockerList User", "dockerlist@example.com")
        .await;
    let org = server.create_org(&user, "DockerList Org").await;
    let project = server
        .create_project(&user, &org, "DockerList Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let manifest = create_docker_manifest_list();

    // PUT with Docker manifest list Content-Type
    let put_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/docker-list-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.list.v2+json",
        )
        .body(manifest.clone())
        .send()
        .await
        .expect("PUT failed");

    assert_eq!(put_resp.status(), StatusCode::CREATED);
    assert!(put_resp.headers().contains_key("docker-content-digest"));

    // GET back and verify round-trip
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/docker-list-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");

    assert_eq!(get_resp.status(), StatusCode::OK);
    let body = get_resp.text().await.expect("Failed to read body");
    let uploaded: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(uploaded, downloaded);
}

// Verify platform entries are preserved in Docker manifest list
#[tokio::test]
async fn test_manifest_get_docker_manifest_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("DockerListGet User", "dockerlistget@example.com")
        .await;
    let org = server.create_org(&user, "DockerListGet Org").await;
    let project = server
        .create_project(&user, &org, "DockerListGet Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let manifest = create_docker_manifest_list();

    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/docker-list-get", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.list.v2+json",
        )
        .body(manifest.clone())
        .send()
        .await
        .expect("PUT failed");

    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/docker-list-get", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");

    assert_eq!(get_resp.status(), StatusCode::OK);

    // Verify content-type
    let get_ct = get_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type")
        .to_str()
        .expect("Invalid content-type");
    assert_eq!(
        get_ct, "application/vnd.docker.distribution.manifest.list.v2+json",
        "Content-type should be Docker manifest list"
    );

    // Verify platform entries are preserved
    let body = get_resp.text().await.expect("Failed to read body");
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    let manifests = downloaded["manifests"]
        .as_array()
        .expect("manifests should be an array");
    assert_eq!(manifests.len(), 2);
    assert_eq!(manifests[0]["platform"]["architecture"], "amd64");
    assert_eq!(manifests[0]["platform"]["os"], "linux");
    assert_eq!(manifests[1]["platform"]["architecture"], "arm64");
    assert_eq!(manifests[1]["platform"]["os"], "linux");
}

// =============================================================================
// OCI Image Index Tests
// =============================================================================

/// Create an OCI image index JSON for testing
fn create_oci_image_index() -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": "sha256:a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
                "size": 1024,
                "platform": {
                    "architecture": "amd64",
                    "os": "linux"
                }
            },
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": "sha256:b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3",
                "size": 1024,
                "platform": {
                    "architecture": "arm64",
                    "os": "linux",
                    "variant": "v8"
                }
            },
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": "sha256:c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
                "size": 1024,
                "platform": {
                    "architecture": "s390x",
                    "os": "linux"
                }
            }
        ]
    })
    .to_string()
}

// PUT multi-platform OCI image index, verify 201
#[tokio::test]
async fn test_manifest_put_oci_image_index() {
    let server = TestServer::new().await;
    let user = server
        .signup("OciIndex User", "ociindexput@example.com")
        .await;
    let org = server.create_org(&user, "OciIndex Org").await;
    let project = server.create_project(&user, &org, "OciIndex Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let manifest = create_oci_image_index();

    let put_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/oci-index-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.index.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("PUT failed");

    assert_eq!(put_resp.status(), StatusCode::CREATED);
    assert!(put_resp.headers().contains_key("location"));
    assert!(put_resp.headers().contains_key("docker-content-digest"));
}

// Verify manifests array and platform entries round-trip for OCI image index
#[tokio::test]
async fn test_manifest_get_oci_image_index() {
    let server = TestServer::new().await;
    let user = server
        .signup("OciIndexGet User", "ociindexget@example.com")
        .await;
    let org = server.create_org(&user, "OciIndexGet Org").await;
    let project = server
        .create_project(&user, &org, "OciIndexGet Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let manifest = create_oci_image_index();

    // Upload
    server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/oci-index-get", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.index.v1+json")
        .body(manifest.clone())
        .send()
        .await
        .expect("PUT failed");

    // GET back
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/oci-index-get", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");

    assert_eq!(get_resp.status(), StatusCode::OK);

    // Verify content-type
    let get_ct = get_resp
        .headers()
        .get("content-type")
        .expect("Missing content-type")
        .to_str()
        .expect("Invalid content-type");
    assert_eq!(
        get_ct, "application/vnd.oci.image.index.v1+json",
        "Content-type should be OCI image index"
    );

    // Verify manifests array and platform entries
    let body = get_resp.text().await.expect("Failed to read body");
    let downloaded: serde_json::Value = serde_json::from_str(&body).unwrap();
    let manifests = downloaded["manifests"]
        .as_array()
        .expect("manifests should be an array");
    assert_eq!(manifests.len(), 3);
    assert_eq!(manifests[0]["platform"]["architecture"], "amd64");
    assert_eq!(manifests[0]["platform"]["os"], "linux");
    assert_eq!(manifests[1]["platform"]["architecture"], "arm64");
    assert_eq!(manifests[1]["platform"]["os"], "linux");
    assert_eq!(manifests[1]["platform"]["variant"], "v8");
    assert_eq!(manifests[2]["platform"]["architecture"], "s390x");
    assert_eq!(manifests[2]["platform"]["os"], "linux");

    // Full round-trip comparison
    let uploaded: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(uploaded, downloaded);
}

// =============================================================================
// Content-Type Validation Tests
// =============================================================================

// OCI body with Docker Content-Type header should return 400
#[tokio::test]
async fn test_manifest_put_content_type_mismatch() {
    let server = TestServer::new().await;
    let user = server
        .signup("CTMismatch User", "ctmismatch@example.com")
        .await;
    let org = server.create_org(&user, "CTMismatch Org").await;
    let project = server
        .create_project(&user, &org, "CTMismatch Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Body contains OCI mediaType
    let manifest = create_test_manifest(
        "sha256:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
        "sha256:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    );

    // But Content-Type header is Docker V2 - mismatch!
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/ct-mismatch-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Mismatched Content-Type header and manifest mediaType should return 400"
    );
}

// PUT /v2/{name}/manifests/{reference} - Manifest referencing blobs that were never uploaded
// The registry validates that referenced blobs exist before storing a manifest.
#[tokio::test]
async fn test_manifest_put_missing_blobs() {
    let server = TestServer::new().await;
    let user = server
        .signup("MissingBlobs User", "missingblobs@example.com")
        .await;
    let org = server.create_org(&user, "MissingBlobs Org").await;
    let project = server
        .create_project(&user, &org, "MissingBlobs Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Reference digests for blobs that were NEVER uploaded
    let config_digest = "sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let layer_digest = "sha256:cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe";
    let manifest = create_test_manifest(config_digest, layer_digest);

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/missing-blobs", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    // Registry rejects manifests with missing blobs (MANIFEST_BLOB_UNKNOWN)
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Manifest referencing non-existent blobs should be rejected"
    );
}

// Matching Content-Type should succeed (sanity check)
#[tokio::test]
async fn test_manifest_put_content_type_match() {
    let server = TestServer::new().await;
    let user = server.signup("CTMatch User", "ctmatch@example.com").await;
    let org = server.create_org(&user, "CTMatch Org").await;
    let project = server.create_project(&user, &org, "CTMatch Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Body and Content-Type both use Docker V2
    let manifest = create_docker_v2_manifest(
        "sha256:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
        "sha256:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    );

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/ct-match-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Matching Content-Type header and manifest mediaType should return 201"
    );
}

// PUT /v2/{name}/manifests/{digest} - Digest in URL does not match manifest content
#[tokio::test]
async fn test_manifest_put_digest_mismatch() {
    let server = TestServer::new().await;
    let user = server
        .signup("DigestMismatch User", "digestmismatch@example.com")
        .await;
    let org = server.create_org(&user, "DigestMismatch Org").await;
    let project = server
        .create_project(&user, &org, "DigestMismatch Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // First upload the blobs so blob validation passes
    let config_data = b"config for digest mismatch test";
    let layer_data = b"layer for digest mismatch test";
    let config_digest = compute_digest(config_data);
    let layer_digest = compute_digest(layer_data);

    server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, config_digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .body(config_data.to_vec())
        .send()
        .await
        .expect("Config upload failed");

    server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, layer_digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .body(layer_data.to_vec())
        .send()
        .await
        .expect("Layer upload failed");

    let manifest = create_test_manifest(&config_digest, &layer_digest);

    // Use a completely wrong digest in the URL
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, wrong_digest)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Manifest PUT with mismatched digest in URL should be rejected"
    );
}

// =============================================================================
// Concurrent Manifest Tag Overwrite Tests
// =============================================================================

// Two concurrent PUTs to the same tag should both succeed and the tag should
// resolve to one of the two manifests without corruption
#[tokio::test]
async fn test_concurrent_manifest_tag_overwrite() {
    let server = TestServer::new().await;
    let user = server
        .signup("ConcurrentTag User", "concurrenttag@example.com")
        .await;
    let org = server.create_org(&user, "ConcurrentTag Org").await;
    let project = server
        .create_project(&user, &org, "ConcurrentTag Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload blobs that the manifests will reference
    let config1_data = b"config data for manifest 1";
    let layer1_data = b"layer data for manifest 1";
    let config2_data = b"config data for manifest 2";
    let layer2_data = b"layer data for manifest 2";

    let config1_digest = compute_digest(config1_data);
    let layer1_digest = compute_digest(layer1_data);
    let config2_digest = compute_digest(config2_data);
    let layer2_digest = compute_digest(layer2_data);

    for (data, digest) in [
        (config1_data.as_slice(), &config1_digest),
        (layer1_data.as_slice(), &layer1_digest),
        (config2_data.as_slice(), &config2_digest),
        (layer2_data.as_slice(), &layer2_digest),
    ] {
        let resp = server
            .client
            .put(server.api_url(&format!(
                "/v2/{}/blobs/uploads?digest={}",
                project_slug, digest
            )))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/octet-stream")
            .body(data.to_vec())
            .send()
            .await
            .expect("Blob upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Create two distinct manifests with different config digests
    let manifest1 = create_test_manifest(&config1_digest, &layer1_digest);
    let manifest2 = create_test_manifest(&config2_digest, &layer2_digest);
    let digest1 = compute_digest(manifest1.as_bytes());
    let digest2 = compute_digest(manifest2.as_bytes());

    // Concurrently PUT both manifests to the same tag
    let url = server.api_url(&format!("/v2/{}/manifests/race-tag", project_slug));
    let (resp1, resp2) = tokio::join!(
        server
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest1.clone())
            .send(),
        server
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest2.clone())
            .send()
    );

    assert_eq!(
        resp1.expect("PUT 1 failed").status(),
        StatusCode::CREATED,
        "First concurrent PUT should succeed"
    );
    assert_eq!(
        resp2.expect("PUT 2 failed").status(),
        StatusCode::CREATED,
        "Second concurrent PUT should succeed"
    );

    // HEAD the tag to get the winning digest
    let head_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/manifests/race-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("HEAD failed");
    assert_eq!(head_resp.status(), StatusCode::OK);

    let returned_digest = head_resp
        .headers()
        .get("docker-content-digest")
        .expect("Missing digest header")
        .to_str()
        .expect("Invalid digest header")
        .to_owned();

    // The tag should point to one of the two manifests
    assert!(
        returned_digest == digest1 || returned_digest == digest2,
        "Tag should resolve to one of the two manifests, got {}",
        returned_digest
    );

    // GET the tag and verify the body is valid JSON and matches the digest
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/race-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("GET failed");
    assert_eq!(get_resp.status(), StatusCode::OK);

    let body = get_resp.bytes().await.expect("Failed to read body");
    let body_digest = compute_digest(&body);
    assert_eq!(
        body_digest, returned_digest,
        "GET body digest should match HEAD digest (no corruption)"
    );

    // Verify the body is valid JSON
    let _parsed: serde_json::Value =
        serde_json::from_slice(&body).expect("GET body should be valid JSON");
}

// =============================================================================
// Max Body Size Tests
// =============================================================================

// Manifest PUT exceeding max body size should be rejected
#[tokio::test]
async fn test_manifest_put_exceeds_max_body_size() {
    // max_body_size = 100 bytes
    let server = TestServer::new_with_limits(3600, 100).await;
    let user = server
        .signup("ManifestLimit User", "manifestlimit@example.com")
        .await;
    let org = server.create_org(&user, "ManifestLimit Org").await;
    let project = server
        .create_project(&user, &org, "ManifestLimit Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // A typical test manifest is ~300 bytes, exceeding the 100-byte limit
    let manifest = create_test_manifest(
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/too-large", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Request failed");

    assert!(
        resp.status().is_client_error(),
        "Manifest PUT exceeding max body size should be rejected, got {}",
        resp.status()
    );
}

// =============================================================================
// Storage Failure Injection Tests
// =============================================================================

// Manifest read after storage directory is deleted
#[tokio::test]
async fn test_manifest_read_after_storage_deleted() {
    let server = TestServer::new().await;
    let user = server
        .signup("StorageFail User", "storagefailmanifest@example.com")
        .await;
    let org = server.create_org(&user, "StorageFail Org").await;
    let project = server
        .create_project(&user, &org, "StorageFail Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload config and layer blobs, then manifest
    let config_data = b"config for storage fail test";
    let layer_data = b"layer for storage fail test";
    let config_digest = compute_digest(config_data);
    let layer_digest = compute_digest(layer_data);

    for (data, digest) in [
        (config_data.as_slice(), &config_digest),
        (layer_data.as_slice(), &layer_digest),
    ] {
        let resp = server
            .client
            .put(server.api_url(&format!(
                "/v2/{}/blobs/uploads?digest={}",
                project_slug, digest
            )))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/octet-stream")
            .body(data.to_vec())
            .send()
            .await
            .expect("Blob upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let manifest = create_test_manifest(&config_digest, &layer_digest);
    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/storage-fail-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Manifest upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Delete project's OCI storage directory
    let project_oci_dir = server
        .db_path()
        .parent()
        .unwrap()
        .join("oci")
        .join(project.uuid.to_string());
    tokio::fs::remove_dir_all(&project_oci_dir)
        .await
        .expect("Failed to delete project OCI storage directory");

    // Try to GET the manifest - should fail with 404
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/storage-fail-tag", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Manifest read after storage deleted should return 404"
    );
}
