#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project measure endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonMeasure, JsonMeasures};
use http::StatusCode;

// GET /v0/projects/{project}/measures - list measures
#[tokio::test]
async fn measures_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "measurelist@example.com").await;
    let org = server.create_org(&user, "Measure Org").await;
    let project = server.create_project(&user, &org, "Measure Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _measures: JsonMeasures = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/measures - create measure
#[tokio::test]
async fn measures_create() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "measurecreate@example.com")
        .await;
    let org = server.create_org(&user, "Measure Create Org").await;
    let project = server
        .create_project(&user, &org, "Measure Create Project")
        .await;

    let body = serde_json::json!({
        "name": "Latency",
        "slug": "latency",
        "units": "ns"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let measure: JsonMeasure = resp.json().await.expect("Failed to parse response");
    assert_eq!(measure.name.as_ref(), "Latency");
}

// POST /v0/projects/{project}/measures - create with auto-generated slug
#[tokio::test]
async fn measures_create_auto_slug() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "measureautoslug@example.com")
        .await;
    let org = server.create_org(&user, "Measure AutoSlug Org").await;
    let project = server
        .create_project(&user, &org, "Measure AutoSlug Project")
        .await;

    // Slug is optional - will be auto-generated from name
    let body = serde_json::json!({
        "name": "Throughput",
        "units": "ops/sec"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// DELETE /v0/projects/{project}/measures/{measure} - delete measure
#[tokio::test]
async fn measures_delete() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "measuredelete@example.com")
        .await;
    let org = server.create_org(&user, "Measure Delete Org").await;
    let project = server
        .create_project(&user, &org, "Measure Delete Project")
        .await;

    let body = serde_json::json!({
        "name": "Delete Measure",
        "slug": "delete-measure",
        "units": "ms"
    });

    let project_slug: &str = project.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/measures/delete-measure",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
