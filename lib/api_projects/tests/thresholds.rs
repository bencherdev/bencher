#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::similar_names,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]
//! Integration tests for project threshold endpoints.

use bencher_api_tests::{TestServer, TestUser};
use bencher_json::{JsonThreshold, JsonThresholds, ModelUuid};
use http::StatusCode;

// GET /v0/projects/{project}/thresholds - list thresholds
#[tokio::test]
async fn thresholds_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdlist@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let thresholds: JsonThresholds = resp.json().await.expect("Failed to parse response");
    // New project should have no thresholds
    assert!(thresholds.0.is_empty());
}

// GET /v0/projects/{project}/thresholds - requires auth
#[tokio::test]
async fn thresholds_list_requires_auth() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdauth@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Auth Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Auth Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .send()
        .await
        .expect("Request failed");

    // Public project can be viewed without auth
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::BAD_REQUEST,
        "Expected OK or BAD_REQUEST, got: {}",
        resp.status()
    );
}

// GET /v0/projects/{project}/thresholds/{threshold} - not found
#[tokio::test]
async fn thresholds_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Threshold NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Threshold NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Helper: create a branch, testbed, measure, and threshold for a project.
/// Returns the created `JsonThreshold`.
async fn create_threshold_with_model(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
) -> JsonThreshold {
    // Create a branch
    let body = serde_json::json!({
        "name": "threshold-branch",
        "slug": "threshold-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a testbed
    let body = serde_json::json!({
        "name": "threshold-testbed",
        "slug": "threshold-testbed"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a measure
    let body = serde_json::json!({
        "name": "throughput",
        "slug": "throughput",
        "units": "ops/sec"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a threshold with a t-test model
    let body = serde_json::json!({
        "branch": "threshold-branch",
        "testbed": "threshold-testbed",
        "measure": "throughput",
        "test": "t_test",
        "max_sample_size": 64,
        "upper_boundary": 0.99
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Failed to parse threshold")
}

// GET /v0/projects/{project}/thresholds/{threshold}?model= - view threshold with specific model
#[tokio::test]
async fn thresholds_get_with_model_query() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodel@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Model Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Model Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create threshold with initial model (model A)
    let threshold = create_threshold_with_model(&server, &user, project_slug).await;
    let model_a = threshold.model.expect("threshold should have a model");
    let model_a_uuid = model_a.uuid;

    // GET without ?model= returns current model (model A)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_a_uuid
    );

    // PUT to update threshold with a new model (model B)
    let update_body = serde_json::json!({
        "test": "percentage",
        "max_sample_size": 32,
        "upper_boundary": 0.05
    });
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&update_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonThreshold = resp
        .json()
        .await
        .expect("Failed to parse updated threshold");
    let model_b = updated
        .model
        .expect("updated threshold should have a model");
    let model_b_uuid = model_b.uuid;
    // New model should be different from old model
    assert_ne!(model_a_uuid, model_b_uuid);

    // GET without ?model= returns current model (model B)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_b_uuid
    );

    // GET with ?model=<model_a_uuid> returns historical model A
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, model_a_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_a_uuid
    );

    // GET with ?model=<model_b_uuid> returns current model B explicitly
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, model_b_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_b_uuid
    );
}

// GET /v0/projects/{project}/thresholds/{threshold}?model=<nonexistent> - returns 404
#[tokio::test]
async fn thresholds_get_with_nonexistent_model() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodelnf@example.com")
        .await;
    let org = server.create_org(&user, "Threshold ModelNF Org").await;
    let project = server
        .create_project(&user, &org, "Threshold ModelNF Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create threshold with a model
    let threshold = create_threshold_with_model(&server, &user, project_slug).await;

    // GET with a nonexistent model UUID should return 404
    let bogus_uuid = ModelUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, bogus_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/thresholds/{threshold}?model=<other_threshold_model> - returns 404
#[tokio::test]
async fn thresholds_get_with_wrong_threshold_model() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodelwrong@example.com")
        .await;
    let org = server.create_org(&user, "Threshold ModelWrong Org").await;
    let project = server
        .create_project(&user, &org, "Threshold ModelWrong Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create first threshold (with its own model)
    let threshold_a = create_threshold_with_model(&server, &user, project_slug).await;
    let model_a_uuid = threshold_a.model.as_ref().expect("model should exist").uuid;

    // Create a second branch/testbed/measure for a second threshold
    let body = serde_json::json!({
        "name": "other-branch",
        "slug": "other-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create second threshold (reuse testbed and measure, different branch)
    let body = serde_json::json!({
        "branch": "other-branch",
        "testbed": "threshold-testbed",
        "measure": "throughput",
        "test": "z_score",
        "max_sample_size": 16,
        "upper_boundary": 0.95
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let threshold_b: JsonThreshold = resp.json().await.expect("Failed to parse threshold B");

    // GET threshold B with threshold A's model UUID should return 404
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold_b.uuid, model_a_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
