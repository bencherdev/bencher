#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix,
    clippy::indexing_slicing
)]
//! Integration tests for OCI tags endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

/// Create a minimal OCI manifest JSON for testing
fn create_test_manifest(suffix: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": format!("sha256:config{}{}", suffix, "0".repeat(56)),
            "size": 100
        },
        "layers": []
    })
    .to_string()
}

// GET /v2/{name}/tags/list - List tags (empty)
#[tokio::test]
async fn test_tags_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Tags User", "tagsempty@example.com").await;
    let org = server.create_org(&user, "Tags Org").await;
    let project = server.create_project(&user, &org, "Tags Project").await;

    let oci_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
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
async fn test_tags_list_with_manifests() {
    let server = TestServer::new().await;
    let user = server.signup("TagsList User", "tagslist@example.com").await;
    let org = server.create_org(&user, "TagsList Org").await;
    let project = server.create_project(&user, &org, "TagsList Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload manifests with different tags
    let tags = ["v1.0.0", "v1.1.0", "latest"];
    for (i, tag) in tags.iter().enumerate() {
        let manifest = create_test_manifest(&i.to_string());
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    // List tags
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
async fn test_tags_list_pagination_n() {
    let server = TestServer::new().await;
    let user = server.signup("TagsPage User", "tagspage@example.com").await;
    let org = server.create_org(&user, "TagsPage Org").await;
    let project = server.create_project(&user, &org, "TagsPage Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 5 manifests
    for i in 0..5 {
        let manifest = create_test_manifest(&format!("page{}", i));
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    // List with n=2
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
async fn test_tags_list_pagination_last() {
    let server = TestServer::new().await;
    let user = server.signup("TagsLast User", "tagslast@example.com").await;
    let org = server.create_org(&user, "TagsLast Org").await;
    let project = server.create_project(&user, &org, "TagsLast Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload manifests with alphabetically ordered tags
    let tags = ["aaa", "bbb", "ccc", "ddd"];
    for (i, tag) in tags.iter().enumerate() {
        let manifest = create_test_manifest(&format!("last{}", i));
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    // List with last=bbb (should return tags after "bbb")
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?last=bbb", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
async fn test_tags_list_unauthenticated() {
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
async fn test_tags_options() {
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
async fn test_tags_list_pagination_link_header() {
    let server = TestServer::new().await;
    let user = server.signup("TagsLink User", "tagslink@example.com").await;
    let org = server.create_org(&user, "TagsLink Org").await;
    let project = server.create_project(&user, &org, "TagsLink Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload 5 manifests with different tags
    for i in 0..5 {
        let manifest = create_test_manifest(&format!("link{}", i));
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    // List with n=2 (should have Link header since there are 5 tags)
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
async fn test_tags_list_no_link_header_when_complete() {
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
        let manifest = create_test_manifest(&format!("nolink{}", i));
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/tag{}", project_slug, i)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    // List with n=5 (larger than number of tags, so no Link header expected)
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=5", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
async fn test_tags_list_follow_pagination() {
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
        let manifest = create_test_manifest(&format!("follow{}", i));
        server
            .client
            .put(server.api_url(&format!("/v2/{}/manifests/{}", project_slug, tag)))
            .header("Authorization", format!("Bearer {}", push_token))
            .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
            .body(manifest)
            .send()
            .await
            .expect("Upload failed");
    }

    let pull_token = server.oci_pull_token(&user, &project);

    // First page: n=2
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/tags/list?n=2", project_slug)))
        .header("Authorization", format!("Bearer {}", pull_token))
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
        .header("Authorization", format!("Bearer {}", pull_token))
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
        .header("Authorization", format!("Bearer {}", pull_token))
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
