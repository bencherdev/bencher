#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix
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
