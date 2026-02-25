//! Integration tests for run endpoint.
#![expect(
    unused_crate_dependencies,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module
)]

use bencher_api_tests::TestServer;
#[cfg(feature = "plus")]
use bencher_api_tests::oci::compute_digest;
use bencher_json::JsonReport;
#[cfg(feature = "plus")]
use bencher_json::{JsonJob, JsonSpec, runner::JsonJobs};
use http::StatusCode;

// POST /v0/run - create a run with authentication
#[tokio::test]
async fn run_post_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runauth@example.com").await;
    let org = server.create_org(&user, "Run Org").await;
    let project = server.create_project(&user, &org, "Run Project").await;

    let project_slug: &str = project.slug.as_ref();
    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let _report: JsonReport = resp.json().await.expect("Failed to parse response");
}

// POST /v0/run - run with existing project creates branch/testbed as needed
#[tokio::test]
async fn run_post_creates_branch_testbed() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runcreate@example.com").await;
    let org = server.create_org(&user, "Run Create Org").await;
    let project = server
        .create_project(&user, &org, "Auto Create Project")
        .await;

    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let project_slug: &str = project.slug.as_ref();
    // Run with new branch and testbed names that don't exist yet
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "feature-branch",
        "testbed": "new-testbed",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should create the branch, testbed, and run successfully
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// POST /v0/run - run without authentication (public run)
#[tokio::test]
async fn run_post_unauthenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runpublic@example.com").await;
    let org = server.create_org(&user, "Public Run Org").await;
    let project = server
        .create_project(&user, &org, "Public Run Project")
        .await;

    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    // Try without authentication - should fail for non-public project
    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Without auth, should get unauthorized or forbidden
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN,
        "Expected auth error, got: {}",
        resp.status()
    );
}

// --- Job creation integration tests (Plus only) ---

#[cfg(feature = "plus")]
fn bmf_results() -> serde_json::Value {
    serde_json::json!({
        "benchmark_name": {
            "latency": { "value": 100.0 }
        }
    })
}

/// Create a minimal OCI image manifest referencing the given config and layer digests.
#[cfg(feature = "plus")]
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

/// Push a test OCI image to the registry and return the manifest digest.
#[cfg(feature = "plus")]
async fn push_test_image(
    server: &TestServer,
    project: &bencher_api_tests::TestProject,
    user: &bencher_api_tests::TestUser,
    tag: &str,
) -> String {
    let project_slug: &str = project.slug.as_ref();
    let oci_token = server.oci_push_token(user, project);

    let config_data = b"config data for job test";
    let config_digest = server
        .upload_blob(project_slug, &oci_token, config_data)
        .await;

    let layer_data = b"layer data for job test";
    let layer_digest = server
        .upload_blob(project_slug, &oci_token, layer_data)
        .await;

    let manifest = create_test_manifest(&config_digest, &layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{project_slug}/manifests/{tag}")))
        .header("Authorization", format!("Bearer {oci_token}"))
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Manifest push failed");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Manifest push should succeed"
    );

    manifest_digest
}

/// Create a fallback spec and return it.
#[cfg(feature = "plus")]
async fn create_fallback_spec(
    server: &TestServer,
    admin: &bencher_api_tests::TestUser,
) -> JsonSpec {
    let body = serde_json::json!({
        "name": "Job Test Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Failed to create spec");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Failed to create fallback spec"
    );
    resp.json().await.expect("Failed to parse spec response")
}

/// List jobs for a project and return them.
#[cfg(feature = "plus")]
async fn list_project_jobs(
    server: &TestServer,
    user: &bencher_api_tests::TestUser,
    project_slug: &str,
) -> Vec<JsonJob> {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Failed to list jobs");
    assert_eq!(resp.status(), StatusCode::OK, "Failed to list project jobs");
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse jobs response");
    jobs.0
}

// POST /v0/run with job + fallback spec creates a pending job
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_creates_job() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob1@example.com").await;
    let org = server.create_org(&user, "Job Org").await;
    let project = server.create_project(&user, &org, "Job Project").await;

    // Create a fallback spec (first user is admin)
    let spec = create_fallback_spec(&server, &user).await;

    // Push OCI image
    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run with job
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1")
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let _report: JsonReport = resp.json().await.expect("Failed to parse response");

    // Verify job was created
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1, "Expected exactly one job");
    assert_eq!(jobs[0].status, bencher_json::JobStatus::Pending);
    assert_eq!(
        AsRef::<str>::as_ref(&jobs[0].spec.slug),
        AsRef::<str>::as_ref(&spec.slug),
    );
}

// POST /v0/run with job + explicit spec creates a job using that spec
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_explicit_spec() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob2@example.com").await;
    let org = server.create_org(&user, "Job Spec Org").await;
    let project = server.create_project(&user, &org, "Job Spec Project").await;

    // Create two specs: one fallback, one non-fallback
    let _fallback = create_fallback_spec(&server, &user).await;

    let body = serde_json::json!({
        "name": "Explicit Spec",
        "architecture": "aarch64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Failed to create spec");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let explicit_spec: JsonSpec = resp.json().await.expect("Failed to parse spec response");

    // Push OCI image
    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v2").await;

    // Submit run with job referencing explicit spec by slug
    let explicit_slug: &str = explicit_spec.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v2"),
            "spec": explicit_slug
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify job uses the explicit spec, not the fallback
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(AsRef::<str>::as_ref(&jobs[0].spec.slug), explicit_slug,);
}

// POST /v0/run with job but no spec and no fallback fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_no_spec_fails() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob3@example.com").await;
    let org = server.create_org(&user, "No Spec Org").await;
    let project = server.create_project(&user, &org, "No Spec Project").await;

    // Push OCI image (no spec created)
    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1")
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should fail because no spec is available
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/run with job referencing unsupported external registry fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_unsupported_registry() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob4@example.com").await;
    let org = server.create_org(&user, "Bad Reg Org").await;
    let project = server.create_project(&user, &org, "Bad Reg Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    // Use an external registry that is not docker.io or localhost
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": "ghcr.io/some-user/some-image:v1"
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/run without job field creates report but no job
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_without_job_no_job_created() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob5@example.com").await;
    let org = server.create_org(&user, "No Job Org").await;
    let project = server.create_project(&user, &org, "No Job Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // No job should be created
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert!(jobs.is_empty(), "Expected no jobs, got {}", jobs.len());
}

// POST /v0/run with job using docker.io image (default registry) passes validation
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_docker_io_image() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob6@example.com").await;
    let org = server.create_org(&user, "Docker Org").await;
    let project = server.create_project(&user, &org, "Docker Project").await;

    create_fallback_spec(&server, &user).await;

    // Push OCI image with tag "v1"
    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Use a bare docker.io image reference. Registry validation passes because
    // docker.io is always allowed. Tag resolution uses the project UUID as
    // the repository key, so the "v1" tag pushed above will resolve.
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": "alpine:v1"
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // docker.io is allowed, and the tag "v1" resolves via the project's OCI storage
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, bencher_json::JobStatus::Pending);
}

// POST /v0/run with job using image digest directly
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_digest_reference() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob7@example.com").await;
    let org = server.create_org(&user, "Digest Org").await;
    let project = server.create_project(&user, &org, "Digest Project").await;

    create_fallback_spec(&server, &user).await;

    // Push OCI image and get the manifest digest
    let project_slug: &str = project.slug.as_ref();
    let manifest_digest = push_test_image(&server, &project, &user, "v1").await;

    // Use the digest directly instead of a tag
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}@{manifest_digest}")
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, bencher_json::JobStatus::Pending);
}

// POST /v0/run with job and custom timeout
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_custom_timeout() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob8@example.com").await;
    let org = server.create_org(&user, "Timeout Org").await;
    let project = server.create_project(&user, &org, "Timeout Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Request a 120s timeout — should be clamped to unclaimed max (300s)
    // since the project is not claimed (no billing plan)
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1"),
            "timeout": 120
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
}
