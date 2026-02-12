#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for project benchmark endpoints.

use bencher_api_tests::TestServer;
use bencher_json::JsonBenchmarks;
use http::StatusCode;

// GET /v0/projects/{project}/benchmarks - list benchmarks (empty)
#[tokio::test]
async fn benchmarks_list_empty() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarklist@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let benchmarks: JsonBenchmarks = resp.json().await.expect("Failed to parse response");
    // New project should have no benchmarks
    assert!(benchmarks.0.is_empty());
}

// GET /v0/projects/{project}/benchmarks - with search
#[tokio::test]
async fn benchmarks_list_with_search() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarksearch@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Search Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Search Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks?search=foo",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/benchmarks/{benchmark} - not found
#[tokio::test]
async fn benchmarks_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarknotfound@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/nonexistent-benchmark",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// DELETE /v0/projects/{project}/benchmarks/{benchmark} - not found
#[tokio::test]
async fn benchmarks_delete_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarkdelnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Del NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Del NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/nonexistent-benchmark",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
