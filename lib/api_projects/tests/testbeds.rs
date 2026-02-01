#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix,
    clippy::uninlined_format_args
)]
//! Integration tests for project testbed endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonTestbed, JsonTestbeds};
use http::StatusCode;

// GET /v0/projects/{project}/testbeds - list testbeds
#[tokio::test]
async fn test_testbeds_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "testbedlist@example.com").await;
    let org = server.create_org(&user, "Testbed Org").await;
    let project = server.create_project(&user, &org, "Testbed Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _testbeds: JsonTestbeds = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/testbeds - create testbed
#[tokio::test]
async fn test_testbeds_create() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "testbedcreate@example.com")
        .await;
    let org = server.create_org(&user, "Testbed Create Org").await;
    let project = server
        .create_project(&user, &org, "Testbed Create Project")
        .await;

    let body = serde_json::json!({
        "name": "linux-server",
        "slug": "linux-server"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse response");
    assert_eq!(testbed.name.as_ref(), "linux-server");
}

// POST /v0/projects/{project}/testbeds - duplicate slug fails
#[tokio::test]
async fn test_testbeds_create_duplicate() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "testbeddup@example.com").await;
    let org = server.create_org(&user, "Testbed Dup Org").await;
    let project = server
        .create_project(&user, &org, "Testbed Dup Project")
        .await;

    let body = serde_json::json!({
        "name": "dup-testbed",
        "slug": "dup-testbed"
    });

    let project_slug: &str = project.slug.as_ref();

    // First creation
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second creation should fail
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// DELETE /v0/projects/{project}/testbeds/{testbed} - delete testbed
#[tokio::test]
async fn test_testbeds_delete() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "testbeddelete@example.com")
        .await;
    let org = server.create_org(&user, "Testbed Delete Org").await;
    let project = server
        .create_project(&user, &org, "Testbed Delete Project")
        .await;

    let body = serde_json::json!({
        "name": "delete-testbed",
        "slug": "delete-testbed"
    });

    let project_slug: &str = project.slug.as_ref();
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/testbeds/delete-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
