#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix
)]
//! Integration tests for OCI upload session endpoints.

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use http::StatusCode;

/// Helper to extract session ID from Location header
fn extract_session_id(location: &str) -> Option<String> {
    // Location format: /v2/{name}/blobs/uploads/{session_id}
    location.split('/').next_back().map(ToOwned::to_owned)
}

// GET /v2/{name}/blobs/uploads/{session_id} - Get upload status
#[tokio::test]
async fn test_upload_status() {
    let server = TestServer::new().await;
    let user = server
        .signup("Status User", "uploadstatus@example.com")
        .await;
    let org = server.create_org(&user, "Status Org").await;
    let project = server.create_project(&user, &org, "Status Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    assert_eq!(start_resp.status(), StatusCode::ACCEPTED);
    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Check upload status (no auth needed - session ID is the auth)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().contains_key("range"));
    assert!(resp.headers().contains_key("docker-upload-uuid"));
}

// PATCH /v2/{name}/blobs/uploads/{session_id} - Upload chunk
#[tokio::test]
async fn test_upload_chunk() {
    let server = TestServer::new().await;
    let user = server.signup("Chunk User", "uploadchunk@example.com").await;
    let org = server.create_org(&user, "Chunk Org").await;
    let project = server.create_project(&user, &org, "Chunk Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload a chunk
    let chunk_data = b"first chunk of data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("range"));

    // Check the range reflects the uploaded data
    let range = resp
        .headers()
        .get("range")
        .expect("Missing range header")
        .to_str()
        .expect("Invalid range header");
    assert!(range.contains(&format!("0-{}", chunk_data.len() - 1)));
}

// PATCH with Content-Range header
#[tokio::test]
async fn test_upload_chunk_with_content_range() {
    let server = TestServer::new().await;
    let user = server.signup("Range User", "uploadrange@example.com").await;
    let org = server.create_org(&user, "Range Org").await;
    let project = server.create_project(&user, &org, "Range Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload first chunk
    let chunk1 = b"first chunk";
    let resp1 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("0-{}", chunk1.len() - 1))
        .body(chunk1.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp1.status(), StatusCode::ACCEPTED);

    // Upload second chunk with correct range
    let chunk2 = b"second chunk";
    let start = chunk1.len();
    let end = start + chunk2.len() - 1;
    let resp2 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("{}-{}", start, end))
        .body(chunk2.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp2.status(), StatusCode::ACCEPTED);
}

// PATCH with wrong Content-Range (should return 416)
#[tokio::test]
async fn test_upload_chunk_wrong_range() {
    let server = TestServer::new().await;
    let user = server
        .signup("WrongRange User", "uploadwrongrange@example.com")
        .await;
    let org = server.create_org(&user, "WrongRange Org").await;
    let project = server
        .create_project(&user, &org, "WrongRange Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Try to upload with wrong range (starting at 100 when nothing uploaded yet)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "100-108")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

// PUT /v2/{name}/blobs/uploads/{session_id}?digest= - Complete upload
#[tokio::test]
async fn test_upload_complete() {
    let server = TestServer::new().await;
    let user = server
        .signup("Complete User", "uploadcomplete@example.com")
        .await;
    let org = server.create_org(&user, "Complete Org").await;
    let project = server.create_project(&user, &org, "Complete Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload chunk
    let chunk = b"complete upload test data";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete the upload
    let digest = compute_digest(chunk);
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, digest
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// PUT with additional data in body
#[tokio::test]
async fn test_upload_complete_with_final_chunk() {
    let server = TestServer::new().await;
    let user = server
        .signup("FinalChunk User", "uploadfinal@example.com")
        .await;
    let org = server.create_org(&user, "FinalChunk Org").await;
    let project = server
        .create_project(&user, &org, "FinalChunk Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload first chunk
    let chunk1 = b"first part ";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete with final chunk in body
    let chunk2 = b"second part";
    let mut full_data = Vec::new();
    full_data.extend_from_slice(chunk1);
    full_data.extend_from_slice(chunk2);
    let digest = compute_digest(&full_data);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, digest
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// PUT with wrong digest (should fail)
#[tokio::test]
async fn test_upload_complete_wrong_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("WrongDigest User", "uploadwrongdigest@example.com")
        .await;
    let org = server.create_org(&user, "WrongDigest Org").await;
    let project = server
        .create_project(&user, &org, "WrongDigest Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload chunk
    let chunk = b"wrong digest test data";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete with wrong digest
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, wrong_digest
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// DELETE /v2/{name}/blobs/uploads/{session_id} - Cancel upload
#[tokio::test]
async fn test_upload_cancel() {
    let server = TestServer::new().await;
    let user = server
        .signup("Cancel User", "uploadcancel@example.com")
        .await;
    let org = server.create_org(&user, "Cancel Org").await;
    let project = server.create_project(&user, &org, "Cancel Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Cancel the upload
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify it's cancelled (status should fail)
    let check_resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(check_resp.status(), StatusCode::NOT_FOUND);
}

// Upload to a cancelled session should return 404
#[tokio::test]
async fn test_upload_to_cancelled_session() {
    let server = TestServer::new().await;
    let user = server
        .signup("Cancelled User", "uploadcancelled@example.com")
        .await;
    let org = server.create_org(&user, "Cancelled Org").await;
    let project = server
        .create_project(&user, &org, "Cancelled Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Cancel the upload
    let cancel_resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Cancel failed");
    assert_eq!(cancel_resp.status(), StatusCode::ACCEPTED);

    // Try to upload a chunk to the cancelled session
    let chunk = b"data after cancel";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Upload to cancelled session should return 404"
    );
}

// PATCH with malformed Content-Range (not parseable)
#[tokio::test]
async fn test_upload_malformed_content_range() {
    let server = TestServer::new().await;
    let user = server
        .signup("MalformedRange User", "uploadmalformedrange@example.com")
        .await;
    let org = server.create_org(&user, "MalformedRange Org").await;
    let project = server
        .create_project(&user, &org, "MalformedRange Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with a completely unparseable Content-Range
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "garbage-value")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    // Content-Range with completely unparseable values on both sides should be ignored
    // (no range validation) and the upload should proceed normally
    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "Unparseable Content-Range should be ignored and upload should succeed"
    );
}

// PATCH with Content-Range including "bytes " prefix (standard HTTP format)
#[tokio::test]
async fn test_upload_content_range_bytes_prefix() {
    let server = TestServer::new().await;
    let user = server
        .signup("BytesPrefix User", "uploadbytesprefix@example.com")
        .await;
    let org = server.create_org(&user, "BytesPrefix Org").await;
    let project = server
        .create_project(&user, &org, "BytesPrefix Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with standard HTTP Content-Range format: "bytes 0-9/*"
    let chunk = b"0123456789";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("bytes 0-{}/*", chunk.len() - 1))
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "Content-Range with 'bytes ' prefix should be accepted"
    );
}

// OPTIONS /v2/{name}/blobs/uploads/{session_id} - CORS preflight
#[tokio::test]
async fn test_upload_session_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/blobs/uploads/some-session-id"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}

// PATCH with Content-Range where only end is unparseable (e.g., "0-abc") should return 416
#[tokio::test]
async fn test_upload_partial_content_range_bad_end() {
    let server = TestServer::new().await;
    let user = server
        .signup("BadEnd User", "uploadbadend@example.com")
        .await;
    let org = server.create_org(&user, "BadEnd Org").await;
    let project = server.create_project(&user, &org, "BadEnd Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with partially unparseable Content-Range (valid start, invalid end)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "0-abc")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Partially unparseable Content-Range (bad end) should return 416"
    );
}

// PATCH with Content-Range where only start is unparseable (e.g., "abc-10") should return 416
#[tokio::test]
async fn test_upload_partial_content_range_bad_start() {
    let server = TestServer::new().await;
    let user = server
        .signup("BadStart User", "uploadbadstart@example.com")
        .await;
    let org = server.create_org(&user, "BadStart Org").await;
    let project = server.create_project(&user, &org, "BadStart Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with partially unparseable Content-Range (invalid start, valid end)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "abc-10")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Partially unparseable Content-Range (bad start) should return 416"
    );
}
