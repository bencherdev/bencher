#![expect(
    unused_crate_dependencies,
    clippy::decimal_literal_representation,
    clippy::similar_names,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]
//! Integration tests for project testbed endpoints.

use bencher_api_tests::TestServer;
#[cfg(feature = "plus")]
use bencher_json::{JsonSpec, SpecUuid};
use bencher_json::{JsonTestbed, JsonTestbeds};
use http::StatusCode;

// GET /v0/projects/{project}/testbeds - list testbeds
#[tokio::test]
async fn testbeds_list() {
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
async fn testbeds_create() {
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
async fn testbeds_create_duplicate() {
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
async fn testbeds_delete() {
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

// GET /v0/projects/{project}/testbeds/{testbed}?spec= - view testbed with specific spec
#[cfg(feature = "plus")]
#[tokio::test]
async fn testbeds_get_with_spec_query() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "testbedspec@example.com").await;
    let org = server.create_org(&user, "Testbed Spec Org").await;
    let project = server
        .create_project(&user, &org, "Testbed Spec Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create two specs
    let spec_a_body = serde_json::json!({
        "name": "Spec A",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&spec_a_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_a: JsonSpec = resp.json().await.expect("Failed to parse spec A");

    let spec_b_body = serde_json::json!({
        "name": "Spec B",
        "architecture": "aarch64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64,
        "network": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&spec_b_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_b: JsonSpec = resp.json().await.expect("Failed to parse spec B");

    // Create a testbed
    let testbed_body = serde_json::json!({
        "name": "spec-testbed",
        "slug": "spec-testbed"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&testbed_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    // Newly created testbed should have no spec
    assert!(testbed.spec.is_none());

    // Set spec A on the testbed
    let patch_body = serde_json::json!({"spec": spec_a.uuid.to_string()});
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/testbeds/spec-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&patch_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec_a.uuid
    );

    // Update testbed to spec B
    let patch_body = serde_json::json!({"spec": spec_b.uuid.to_string()});
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/testbeds/spec-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&patch_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec_b.uuid
    );

    // GET without ?spec= returns current spec (spec B)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/testbeds/spec-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec_b.uuid
    );

    // GET with ?spec=<spec_a_uuid> returns historical spec A
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/testbeds/spec-testbed?spec={}",
            project_slug, spec_a.uuid
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec_a.uuid
    );

    // GET with ?spec=<spec_b_uuid> returns current spec B explicitly
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/testbeds/spec-testbed?spec={}",
            project_slug, spec_b.uuid
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec_b.uuid
    );
}

// GET /v0/projects/{project}/testbeds/{testbed}?spec=<nonexistent> - returns 404
#[cfg(feature = "plus")]
#[tokio::test]
async fn testbeds_get_with_nonexistent_spec() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "testbedspecnf@example.com")
        .await;
    let org = server.create_org(&user, "Testbed SpecNF Org").await;
    let project = server
        .create_project(&user, &org, "Testbed SpecNF Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create a testbed
    let testbed_body = serde_json::json!({
        "name": "specnf-testbed",
        "slug": "specnf-testbed"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&testbed_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // GET with a nonexistent spec UUID should return 404
    let bogus_uuid = SpecUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/testbeds/specnf-testbed?spec={}",
            project_slug, bogus_uuid
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/projects/{project}/testbeds - create testbed with spec in body
#[cfg(feature = "plus")]
#[tokio::test]
async fn testbeds_create_with_spec() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "testbedcreatespec@example.com")
        .await;
    let org = server.create_org(&user, "Testbed CreateSpec Org").await;
    let project = server
        .create_project(&user, &org, "Testbed CreateSpec Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create a spec
    let spec_body = serde_json::json!({
        "name": "Create Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&spec_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse spec");

    // Create a testbed with spec in the body
    let testbed_body = serde_json::json!({
        "name": "testbed-with-spec",
        "slug": "testbed-with-spec",
        "spec": spec.uuid.to_string()
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&testbed_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec.uuid
    );
}

// PATCH /v0/projects/{project}/testbeds/{testbed} - patch with null spec removes it
#[cfg(feature = "plus")]
#[tokio::test]
async fn testbeds_patch_spec_null() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "testbedpatchnull@example.com")
        .await;
    let org = server.create_org(&user, "Testbed PatchNull Org").await;
    let project = server
        .create_project(&user, &org, "Testbed PatchNull Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create a spec
    let spec_body = serde_json::json!({
        "name": "Null Spec",
        "architecture": "aarch64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64,
        "network": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&spec_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse spec");

    // Create a testbed
    let testbed_body = serde_json::json!({
        "name": "null-spec-testbed",
        "slug": "null-spec-testbed"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&testbed_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Set spec on testbed
    let patch_body = serde_json::json!({"spec": spec.uuid.to_string()});
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/testbeds/null-spec-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&patch_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert_eq!(
        testbed.spec.as_ref().expect("spec should be set").uuid,
        spec.uuid
    );

    // Patch with null to remove spec
    let patch_body = serde_json::json!({"spec": null});
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/testbeds/null-spec-testbed",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .json(&patch_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");
    assert!(testbed.spec.is_none());
}
