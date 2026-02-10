#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix,
    clippy::indexing_slicing
)]
//! Integration tests for OCI referrers endpoint.

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use http::StatusCode;

/// Create a base manifest (the subject that will be referenced)
fn create_base_manifest(config_digest: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest,
            "size": 100
        },
        "layers": []
    })
    .to_string()
}

/// Create a referrer manifest that points to a subject
fn create_referrer_manifest(
    config_digest: &str,
    subject_digest: &str,
    artifact_type: &str,
) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "artifactType": artifact_type,
        "config": {
            "mediaType": "application/vnd.oci.empty.v1+json",
            "digest": config_digest,
            "size": 2
        },
        "layers": [],
        "subject": {
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "digest": subject_digest,
            "size": 200
        }
    })
    .to_string()
}

// GET /v2/{name}/referrers/{digest} - List referrers (empty)
#[tokio::test]
async fn test_referrers_list_empty() {
    let server = TestServer::new().await;
    let user = server
        .signup("Referrers User", "referrersempty@example.com")
        .await;
    let org = server.create_org(&user, "Referrers Org").await;
    let project = server
        .create_project(&user, &org, "Referrers Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload config blob and base manifest
    let config_digest = server
        .upload_blob(project_slug, &push_token, b"base config empty")
        .await;
    let base_manifest = create_base_manifest(&config_digest);
    let base_digest = compute_digest(base_manifest.as_bytes());

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/base", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(base_manifest)
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // List referrers (should be empty)
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/{}", project_slug, base_digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(body["schemaVersion"], 2);
    assert_eq!(body["mediaType"], "application/vnd.oci.image.index.v1+json");
    assert!(
        body["manifests"]
            .as_array()
            .expect("manifests should be array")
            .is_empty()
    );
}

// GET /v2/{name}/referrers/{digest} - List referrers with results
#[tokio::test]
async fn test_referrers_list_with_results() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersList User", "referrerslist@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersList Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersList Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload config blob and base manifest
    let base_config_digest = server
        .upload_blob(project_slug, &push_token, b"base config list")
        .await;
    let base_manifest = create_base_manifest(&base_config_digest);
    let base_digest = compute_digest(base_manifest.as_bytes());

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/subject", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(base_manifest)
        .send()
        .await
        .expect("Upload base failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Upload config blob for referrer manifests
    let referrer_config_digest = server.upload_blob(project_slug, &push_token, b"{}").await;

    // Upload referrer manifests
    let artifact_types = [
        "application/vnd.example.sbom",
        "application/vnd.example.sig",
    ];
    for artifact_type in &artifact_types {
        let referrer =
            create_referrer_manifest(&referrer_config_digest, &base_digest, artifact_type);
        let resp = server
            .client
            .put(server.api_url(&format!(
                "/v2/{}/manifests/{}",
                project_slug,
                compute_digest(referrer.as_bytes())
            )))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(referrer)
            .send()
            .await
            .expect("Upload referrer failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List referrers
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/{}", project_slug, base_digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let manifests = body["manifests"]
        .as_array()
        .expect("manifests should be array");
    assert_eq!(manifests.len(), 2);

    // Check artifact types are present
    let types: Vec<&str> = manifests
        .iter()
        .filter_map(|m| m["artifactType"].as_str())
        .collect();
    assert!(types.contains(&"application/vnd.example.sbom"));
    assert!(types.contains(&"application/vnd.example.sig"));
}

// GET /v2/{name}/referrers/{digest}?artifactType= - Filter by artifact type
#[tokio::test]
async fn test_referrers_filter_by_artifact_type() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersFilter User", "referrersfilter@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersFilter Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersFilter Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload config blob and base manifest
    let base_config_digest = server
        .upload_blob(project_slug, &push_token, b"base config filter")
        .await;
    let base_manifest = create_base_manifest(&base_config_digest);
    let base_digest = compute_digest(base_manifest.as_bytes());

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/filter-subject", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(base_manifest)
        .send()
        .await
        .expect("Upload base failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Upload config blob for referrer manifests
    let referrer_config_digest = server.upload_blob(project_slug, &push_token, b"{}").await;

    // Upload referrer manifests with different artifact types
    let sbom_referrer = create_referrer_manifest(
        &referrer_config_digest,
        &base_digest,
        "application/vnd.example.sbom",
    );
    let sbom_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug,
            compute_digest(sbom_referrer.as_bytes())
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(sbom_referrer)
        .send()
        .await
        .expect("Upload sbom referrer failed");
    assert_eq!(sbom_resp.status(), StatusCode::CREATED);

    let sig_referrer = create_referrer_manifest(
        &referrer_config_digest,
        &base_digest,
        "application/vnd.example.sig",
    );
    let sig_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug,
            compute_digest(sig_referrer.as_bytes())
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(sig_referrer)
        .send()
        .await
        .expect("Upload sig referrer failed");
    assert_eq!(sig_resp.status(), StatusCode::CREATED);

    // Filter by SBOM artifact type
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/referrers/{}?artifactType=application/vnd.example.sbom",
            project_slug, base_digest
        )))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // Check OCI-Filters-Applied header
    let filters_applied = resp
        .headers()
        .get("oci-filters-applied")
        .expect("Missing OCI-Filters-Applied header")
        .to_str()
        .expect("Invalid header");
    assert_eq!(filters_applied, "artifactType");

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let manifests = body["manifests"]
        .as_array()
        .expect("manifests should be array");
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0]["artifactType"], "application/vnd.example.sbom");
}

// GET /v2/{name}/referrers/{digest} - Unauthenticated (should fail)
#[tokio::test]
async fn test_referrers_unauthenticated() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersUnauth User", "referrersunauth@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersUnauth Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersUnauth Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let fake_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/{}", project_slug, fake_digest)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// GET /v2/{name}/referrers/{digest} - Invalid digest format
#[tokio::test]
async fn test_referrers_invalid_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersInvalid User", "referrersinvalid@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersInvalid Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersInvalid Project")
        .await;

    let oci_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/invalid-digest", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// OPTIONS /v2/{name}/referrers/{digest} - CORS preflight
#[tokio::test]
async fn test_referrers_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/referrers/sha256:abc123"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}

// GET /v2/{name}/referrers/{digest} - Referrers for a non-existent subject should return empty list
#[tokio::test]
async fn test_referrers_nonexistent_subject() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersNoSubject User", "referrersnosubject@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersNoSubject Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersNoSubject Project")
        .await;

    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Query referrers for a digest that has never had any manifest uploaded
    let nonexistent_digest =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/referrers/{}",
            project_slug, nonexistent_digest
        )))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(body["schemaVersion"], 2);
    assert_eq!(body["mediaType"], "application/vnd.oci.image.index.v1+json");
    assert!(
        body["manifests"]
            .as_array()
            .expect("manifests should be array")
            .is_empty(),
        "Referrers for non-existent subject should return empty list"
    );
}

// DELETE /v2/{name}/manifests/{digest} - Verify referrer link cleanup
#[tokio::test]
async fn test_referrers_cleanup_on_manifest_delete() {
    let server = TestServer::new().await;
    let user = server
        .signup("ReferrersCleanup User", "referrerscleanup@example.com")
        .await;
    let org = server.create_org(&user, "ReferrersCleanup Org").await;
    let project = server
        .create_project(&user, &org, "ReferrersCleanup Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload config blob and base manifest (the subject)
    let base_config_digest = server
        .upload_blob(project_slug, &push_token, b"base config cleanup")
        .await;
    let base_manifest = create_base_manifest(&base_config_digest);
    let base_digest = compute_digest(base_manifest.as_bytes());

    let upload_resp = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/cleanup-subject", project_slug)))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(base_manifest)
        .send()
        .await
        .expect("Upload base failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Upload config blob for referrer manifest
    let referrer_config_digest = server.upload_blob(project_slug, &push_token, b"{}").await;

    // Upload a referrer manifest
    let referrer = create_referrer_manifest(
        &referrer_config_digest,
        &base_digest,
        "application/vnd.example.attestation",
    );
    let referrer_digest = compute_digest(referrer.as_bytes());

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, referrer_digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(referrer)
        .send()
        .await
        .expect("Upload referrer failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify the referrer appears in the list
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/{}", project_slug, base_digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("List referrers failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let manifests = body["manifests"]
        .as_array()
        .expect("manifests should be array");
    assert_eq!(manifests.len(), 1);
    assert_eq!(
        manifests[0]["artifactType"],
        "application/vnd.example.attestation"
    );

    // Delete the referrer manifest by digest
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/manifests/{}",
            project_slug, referrer_digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .send()
        .await
        .expect("Delete referrer failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify the referrer is cleaned up from the list
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/referrers/{}", project_slug, base_digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("List referrers failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let manifests = body["manifests"]
        .as_array()
        .expect("manifests should be array");
    assert!(
        manifests.is_empty(),
        "Referrer link should be cleaned up after manifest deletion"
    );
}
