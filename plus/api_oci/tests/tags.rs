#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]
//! Integration tests for OCI tags endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

/// Create a minimal OCI manifest JSON for testing
fn create_test_manifest(config_digest: &str) -> String {
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

/// Upload a config blob and create a manifest referencing it. Returns the manifest JSON string.
async fn upload_blob_and_create_manifest(
    server: &TestServer,
    project_slug: &str,
    push_token: &str,
    suffix: &str,
) -> String {
    let config_digest = server
        .upload_blob(
            project_slug,
            push_token,
            format!("config-{suffix}").as_bytes(),
        )
        .await;
    create_test_manifest(&config_digest)
}

// GET /v2/{name}/tags/list - List tags (empty)
#[tokio::test]
async fn tags_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Tags User", "tagsempty@example.com").await;
    let org = server.create_org(&user, "Tags Org").await;
    let project = server.create_project(&user, &org, "Tags Project").await;

    let oci_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], project_slug);
    assert!(
        body["tags"]
            .as_array()
            .expect("tags should be array")
            .is_empty()
    );
}

// GET /v2/{name}/tags/list - List tags with manifests
#[tokio::test]
async fn tags_list_with_manifests() {
    let server = TestServer::new().await;
    let user = server.signup("TagsList User", "tagslist@example.com").await;
    let org = server.create_org(&user, "TagsList Org").await;
    let project = server.create_project(&user, &org, "TagsList Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload manifests with different tags
    let tags = ["v1.0.0", "v1.1.0", "latest"];
    for (i, tag) in tags.iter().enumerate() {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("list{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List tags
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags_array.len(), 3);

    // Check all tags are present
    let tag_strings: Vec<&str> = tags_array
        .iter()
        .map(|v| v.as_str().expect("tag should be string"))
        .collect();
    for tag in &tags {
        assert!(tag_strings.contains(tag), "Missing tag: {}", tag);
    }
}

// GET /v2/{name}/tags/list - Pagination with n parameter
#[tokio::test]
async fn tags_list_pagination_n() {
    let server = TestServer::new().await;
    let user = server.signup("TagsPage User", "tagspage@example.com").await;
    let org = server.create_org(&user, "TagsPage Org").await;
    let project = server.create_project(&user, &org, "TagsPage Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 5 manifests
    for i in 0..5 {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("page{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List with n=2
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags_array.len(), 2);
}

// GET /v2/{name}/tags/list - Pagination with last parameter
#[tokio::test]
async fn tags_list_pagination_last() {
    let server = TestServer::new().await;
    let user = server.signup("TagsLast User", "tagslast@example.com").await;
    let org = server.create_org(&user, "TagsLast Org").await;
    let project = server.create_project(&user, &org, "TagsLast Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload manifests with alphabetically ordered tags
    let tags = ["aaa", "bbb", "ccc", "ddd"];
    for (i, tag) in tags.iter().enumerate() {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("last{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List with last=bbb (should return tags after "bbb")
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?last=bbb", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");

    // Should contain tags after "bbb" (ccc, ddd)
    let tag_strings: Vec<&str> = tags_array
        .iter()
        .map(|v| v.as_str().expect("tag should be string"))
        .collect();
    assert!(!tag_strings.contains(&"aaa"));
    assert!(!tag_strings.contains(&"bbb"));
}

// GET /v2/{name}/tags/list - Unauthenticated (should fail)
#[tokio::test]
async fn tags_list_unauthenticated() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsUnauth User", "tagsunauth@example.com")
        .await;
    let org = server.create_org(&user, "TagsUnauth Org").await;
    let project = server
        .create_project(&user, &org, "TagsUnauth Project")
        .await;

    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list", project_slug)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// OPTIONS /v2/{name}/tags/list - CORS preflight
#[tokio::test]
async fn tags_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/tags/list"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}

// GET /v2/{name}/tags/list - Link header present when more results exist
#[tokio::test]
async fn tags_list_pagination_link_header() {
    let server = TestServer::new().await;
    let user = server.signup("TagsLink User", "tagslink@example.com").await;
    let org = server.create_org(&user, "TagsLink Org").await;
    let project = server.create_project(&user, &org, "TagsLink Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 5 manifests with different tags
    for i in 0..5 {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("link{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List with n=2 (should have Link header since there are 5 tags)
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // Check Link header is present
    let link_header = resp
        .headers()
        .get("link")
        .expect("Link header should be present when more results exist");
    let link_str = link_header
        .to_str()
        .expect("Link header should be valid string");

    // Verify Link header format
    assert!(
        link_str.contains("rel=\"next\""),
        "Link header should contain rel=\"next\""
    );
    assert!(
        link_str.contains(&format!("/v2/{}/tags/list", project_slug)),
        "Link header should contain the tags/list path"
    );
    assert!(
        link_str.contains("n=2"),
        "Link header should contain n parameter"
    );
    assert!(
        link_str.contains("last="),
        "Link header should contain last parameter"
    );

    // Check Access-Control-Expose-Headers includes Link
    let expose_headers = resp
        .headers()
        .get("access-control-expose-headers")
        .expect("Access-Control-Expose-Headers should be present");
    assert!(
        expose_headers
            .to_str()
            .expect("header should be valid string")
            .contains("Link"),
        "Access-Control-Expose-Headers should include Link"
    );

    // Verify response body still works
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags_array.len(), 2);
}

// GET /v2/{name}/tags/list - No Link header when all results fit
#[tokio::test]
async fn tags_list_no_link_header_when_complete() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsNoLink User", "tagsnolink@example.com")
        .await;
    let org = server.create_org(&user, "TagsNoLink Org").await;
    let project = server
        .create_project(&user, &org, "TagsNoLink Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload only 2 manifests
    for i in 0..2 {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("nolink{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List with n=5 (larger than number of tags, so no Link header expected)
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=5", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // Link header should NOT be present when all results fit
    assert!(
        resp.headers().get("link").is_none(),
        "Link header should not be present when all results fit in one page"
    );

    // Verify all tags are returned
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags_array.len(), 2);
}

// GET /v2/{name}/tags/list - Follow pagination via Link header
#[tokio::test]
async fn tags_list_follow_pagination() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsFollow User", "tagsfollow@example.com")
        .await;
    let org = server.create_org(&user, "TagsFollow Org").await;
    let project = server
        .create_project(&user, &org, "TagsFollow Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 5 manifests with alphabetically ordered tags
    let tags = ["aaa", "bbb", "ccc", "ddd", "eee"];
    for (i, tag) in tags.iter().enumerate() {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("follow{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let pull_token = server.oci_pull_token(&user, &project);

    // First page: n=2
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let link_header = resp
        .headers()
        .get("link")
        .expect("Link header should be present")
        .to_str()
        .expect("Link header should be valid string")
        .to_owned();
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let page1_tags: Vec<&str> = body["tags"]
        .as_array()
        .expect("tags should be array")
        .iter()
        .map(|v| v.as_str().expect("tag should be string"))
        .collect();
    assert_eq!(page1_tags, vec!["aaa", "bbb"]);

    // Extract the URL from Link header
    // Format: </v2/project/tags/list?n=2&last=bbb>; rel="next"
    let link_url = link_header
        .trim_start_matches('<')
        .split('>')
        .next()
        .expect("Link header should have URL");

    // Second page: follow the Link header
    let resp = server
        .client
        .get(server.api_url(link_url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // Extract Link header before consuming response with json()
    let link_header = resp
        .headers()
        .get("link")
        .expect("Link header should be present")
        .to_str()
        .expect("Link header should be valid string")
        .to_owned();

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let page2_tags: Vec<&str> = body["tags"]
        .as_array()
        .expect("tags should be array")
        .iter()
        .map(|v| v.as_str().expect("tag should be string"))
        .collect();
    assert_eq!(page2_tags, vec!["ccc", "ddd"]);

    // Third page: should have only one tag remaining
    let link_url = link_header
        .trim_start_matches('<')
        .split('>')
        .next()
        .expect("Link header should have URL");

    let resp = server
        .client
        .get(server.api_url(link_url))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // No more Link header since this is the last page
    assert!(
        resp.headers().get("link").is_none(),
        "Link header should not be present on the last page"
    );

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let page3_tags: Vec<&str> = body["tags"]
        .as_array()
        .expect("tags should be array")
        .iter()
        .map(|v| v.as_str().expect("tag should be string"))
        .collect();
    assert_eq!(page3_tags, vec!["eee"]);
}

// GET /v2/{name}/tags/list?n=0 - n=0 should be clamped to 1
#[tokio::test]
async fn tags_list_pagination_n_zero_clamped() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsNZero User", "tagsnzero@example.com")
        .await;
    let org = server.create_org(&user, "TagsNZero Org").await;
    let project = server
        .create_project(&user, &org, "TagsNZero Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 3 manifests
    for i in 0..3 {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("nzero{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Request with n=0 — should clamp to 1, returning exactly 1 tag
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=0", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(
        tags_array.len(),
        1,
        "n=0 should be clamped to 1, returning exactly 1 tag"
    );
}

// GET /v2/{name}/tags/list?n=999999 - large n should be clamped to MAX_PAGE_SIZE
#[tokio::test]
async fn tags_list_pagination_n_large_clamped() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsNLarge User", "tagsnlarge@example.com")
        .await;
    let org = server.create_org(&user, "TagsNLarge Org").await;
    let project = server
        .create_project(&user, &org, "TagsNLarge Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 3 manifests
    for i in 0..3 {
        let manifest = upload_blob_and_create_manifest(
            &server,
            project_slug,
            &push_token,
            &format!("nlarge{i}"),
        )
        .await;
        let resp = server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&push_token),
            )
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Request with n=999999 — should succeed (clamped internally) and return all 3
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=999999", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);

    // Check no Link header before consuming resp with json()
    let has_link = resp.headers().get("link").is_some();
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");
    let tags_array = body["tags"].as_array().expect("tags should be array");
    assert_eq!(tags_array.len(), 3);
    assert!(!has_link, "No Link header since all tags fit");
}

// GET /v2/{name}/tags/list?last=<invalid> - invalid cursor should be rejected
#[tokio::test]
async fn tags_list_invalid_last_cursor() {
    let server = TestServer::new().await;
    let user = server
        .signup("TagsBadCursor User", "tagsbadcursor@example.com")
        .await;
    let org = server.create_org(&user, "TagsBadCursor Org").await;
    let project = server
        .create_project(&user, &org, "TagsBadCursor Project")
        .await;

    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // A tag starting with '-' is invalid per OCI spec
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?last=-invalid-tag", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Invalid last cursor should be rejected"
    );
}

// GET /v2/{name}/tags/list?n=-1 - Negative n should be rejected (u32 can't be negative)
#[tokio::test]
async fn tags_list_pagination_n_negative() {
    let server = TestServer::new().await;
    let user = server.signup("TagsNNeg User", "tagsnneg@example.com").await;
    let org = server.create_org(&user, "TagsNNeg Org").await;
    let project = server.create_project(&user, &org, "TagsNNeg Project").await;

    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=-1", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Negative n should be rejected"
    );
}

// GET /v2/{name}/tags/list?n=abc - Non-integer n should be rejected
#[tokio::test]
async fn tags_list_pagination_n_non_integer() {
    let server = TestServer::new().await;
    let user = server.signup("TagsNStr User", "tagsnstr@example.com").await;
    let org = server.create_org(&user, "TagsNStr Org").await;
    let project = server.create_project(&user, &org, "TagsNStr Project").await;

    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=abc", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Non-integer n should be rejected"
    );
}
