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
use bencher_json::{JsonJob, JsonReports, JsonRunners, JsonSpec, runner::JsonJobs};
use http::StatusCode;

// POST /v0/run - create a run with authentication
#[tokio::test]
async fn run_post_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runauth@example.com").await;
    let org = server.create_org(&user, "Run Org").await;
    let project = server.create_project(&user, &org, "Run Project").await;

    let project_slug: &str = project.slug.as_ref();
    let bmf_results = bmf_results();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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

    let bmf_results = bmf_results();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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

    let bmf_results = bmf_results();

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: JsonReport = resp.json().await.expect("Failed to parse response");
    assert_eq!(AsRef::<str>::as_ref(&report.testbed.name), "localhost");

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

    let explicit_spec = create_spec(&server, &user, "Explicit Spec").await;

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
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
    // docker.io is always allowed. Tag resolution ignores the repository path
    // in the image reference and uses only the project UUID and tag name, so
    // the "v1" tag pushed above (under this project) will resolve regardless
    // of the "alpine" repository path.
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // docker.io is allowed, and the tag "v1" resolves because tag lookup
    // is scoped to the project UUID, not the image repository path
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, bencher_json::JobStatus::Pending);
}

/// Create a non-fallback spec with the given name and return it.
#[cfg(feature = "plus")]
async fn create_spec(
    server: &TestServer,
    admin: &bencher_api_tests::TestUser,
    name: &str,
) -> JsonSpec {
    let body = serde_json::json!({
        "name": name,
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Failed to create spec");
    assert_eq!(resp.status(), StatusCode::CREATED, "Failed to create spec");
    resp.json().await.expect("Failed to parse spec response")
}

// POST /v0/run with job + no testbed + fallback spec → testbed named after spec
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_derived_testbed_uses_spec_name() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_derived1@example.com")
        .await;
    let org = server.create_org(&user, "Derived TB Org").await;
    let project = server
        .create_project(&user, &org, "Derived TB Project")
        .await;

    let spec = create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run WITHOUT "testbed" field, WITH "job"
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: JsonReport = resp.json().await.expect("Failed to parse response");

    // Testbed name should be derived from the spec's name
    assert_eq!(
        AsRef::<str>::as_ref(&report.testbed.name),
        AsRef::<str>::as_ref(&spec.name),
        "Testbed name should match the fallback spec's name"
    );
}

// POST /v0/run with job + explicit testbed + fallback spec → testbed keeps user's name
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_explicit_testbed_keeps_name() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_explicit1@example.com")
        .await;
    let org = server.create_org(&user, "Explicit TB Org").await;
    let project = server
        .create_project(&user, &org, "Explicit TB Project")
        .await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run WITH "testbed" field, WITH "job"
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "my-custom-testbed",
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: JsonReport = resp.json().await.expect("Failed to parse response");

    assert_eq!(
        AsRef::<str>::as_ref(&report.testbed.name),
        "my-custom-testbed",
        "Testbed name should be the user's explicit value"
    );
}

// POST /v0/run with job + no testbed + explicit spec → testbed named after explicit spec
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_derived_testbed_explicit_spec() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_derived2@example.com")
        .await;
    let org = server.create_org(&user, "Derived Explicit Org").await;
    let project = server
        .create_project(&user, &org, "Derived Explicit Project")
        .await;

    create_fallback_spec(&server, &user).await;
    let explicit_spec = create_spec(&server, &user, "Explicit Spec").await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    let explicit_slug: &str = explicit_spec.slug.as_ref();
    // Submit run WITHOUT "testbed" field, WITH "job" referencing explicit spec
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1"),
            "spec": explicit_slug
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: JsonReport = resp.json().await.expect("Failed to parse response");

    assert_eq!(
        AsRef::<str>::as_ref(&report.testbed.name),
        AsRef::<str>::as_ref(&explicit_spec.name),
        "Testbed name should match the explicit spec's name"
    );
}

// POST /v0/run with job + explicit testbed with existing spec → uses testbed's spec
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_explicit_testbed_existing_spec() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_existing1@example.com")
        .await;
    let org = server.create_org(&user, "Existing Spec Org").await;
    let project = server
        .create_project(&user, &org, "Existing Spec Project")
        .await;

    let spec_a = create_spec(&server, &user, "Spec A").await;
    let _spec_b = create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    let spec_a_slug: &str = spec_a.slug.as_ref();
    // First run: create testbed "my-tb" with spec A assigned via explicit --spec
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "my-tb",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1"),
            "spec": spec_a_slug
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify first job uses spec A
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(AsRef::<str>::as_ref(&jobs[0].spec.slug), spec_a_slug,);

    // Second run: same testbed "my-tb", no --spec
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "my-tb",
        "start_time": "2024-01-01T00:02:00Z",
        "end_time": "2024-01-01T00:03:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1")
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second run should use spec A (from testbed), not spec B (fallback)
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        AsRef::<str>::as_ref(&jobs[1].spec.slug),
        spec_a_slug,
        "Second run should use testbed's existing spec A, not fallback"
    );
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

    // Request a 120s timeout — stored as-is since 120s < free max (900s)
    // (the user is authenticated and org is claimed, so free max applies, but no clamping needed)
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);

    // Verify timeout stored correctly (120s < free max 900s, so unchanged)
    {
        use bencher_schema::schema;
        use diesel::{QueryDsl as _, RunQueryDsl as _};
        let mut conn = server.db_conn();
        let stored_timeout: bencher_json::Timeout = schema::job::table
            .select(schema::job::timeout)
            .first(&mut conn)
            .expect("Failed to query job timeout");
        assert_eq!(
            stored_timeout,
            bencher_json::Timeout::try_from(120).unwrap()
        );
    }
}

// POST /v0/run with explicit testbed, no job, with fallback spec → OK, no job created
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_explicit_testbed_no_job_with_fallback_spec() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "run_explicit_nojob_fb@example.com")
        .await;
    let org = server.create_org(&user, "Explicit NoJob FB Org").await;
    let project = server
        .create_project(&user, &org, "Explicit NoJob FB Project")
        .await;

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // No job should be created even though a fallback spec exists
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert!(jobs.is_empty(), "Expected no jobs, got {}", jobs.len());
}

// POST /v0/run with derived testbed (no --testbed), no job, no fallback → OK, no job
// This is the regression test for the bug where RunTestbed::Derived incorrectly
// entered the job resolution path even without a job being requested.
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_no_testbed_no_job_succeeds() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "run_derived_nojob@example.com")
        .await;
    let org = server.create_org(&user, "Derived NoJob Org").await;
    let project = server
        .create_project(&user, &org, "Derived NoJob Project")
        .await;

    // No spec, no fallback — just a plain run without --testbed and without --job
    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert!(jobs.is_empty(), "Expected no jobs, got {}", jobs.len());
}

// POST /v0/run with derived testbed (no --testbed), no job, with fallback → OK, no job
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_no_testbed_no_job_with_fallback_spec_succeeds() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "run_derived_nojob_fb@example.com")
        .await;
    let org = server.create_org(&user, "Derived NoJob FB Org").await;
    let project = server
        .create_project(&user, &org, "Derived NoJob FB Project")
        .await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // No job should be created even though a fallback spec exists
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert!(jobs.is_empty(), "Expected no jobs, got {}", jobs.len());
}

// POST /v0/run with derived testbed (no --testbed), with job, no spec, no fallback → 400
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_derived_no_spec_fails() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "run_derived_job_nospec@example.com")
        .await;
    let org = server.create_org(&user, "Derived Job NoSpec Org").await;
    let project = server
        .create_project(&user, &org, "Derived Job NoSpec Project")
        .await;

    // Push OCI image (no spec created)
    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run WITHOUT testbed, WITH job, no spec and no fallback
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should fail because no spec is available
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/run with job when no runners exist — job stays Pending
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_no_runners_stays_pending() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_norunners@example.com")
        .await;
    let org = server.create_org(&user, "No Runners Org").await;
    let project = server
        .create_project(&user, &org, "No Runners Project")
        .await;

    create_fallback_spec(&server, &user).await;

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify no runners exist
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Failed to list runners");
    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse runners response");
    assert!(runners.0.is_empty(), "Expected no runners");

    // Verify job is still Pending (not claimed, not failed)
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1, "Expected exactly one job");
    assert_eq!(
        jobs[0].status,
        bencher_json::JobStatus::Pending,
        "Job should remain Pending with no runners"
    );
}

// POST /v0/run with invalid image reference fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_invalid_image_fails() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_invalid_img@example.com")
        .await;
    let org = server.create_org(&user, "Invalid Img Org").await;
    let project = server
        .create_project(&user, &org, "Invalid Img Project")
        .await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    // Submit run with an invalid image reference (empty string)
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": ""
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/run with unsupported registry — validation fails before report creation
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_validation_failure_no_report() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_val_fail@example.com")
        .await;
    let org = server.create_org(&user, "Val Fail Org").await;
    let project = server.create_project(&user, &org, "Val Fail Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    // Use an external registry that is not supported
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Verify no report was created
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Failed to list reports");
    assert_eq!(resp.status(), StatusCode::OK);
    let reports: JsonReports = resp.json().await.expect("Failed to parse reports response");
    assert!(
        reports.0.is_empty(),
        "Expected no reports after validation failure, got {}",
        reports.0.len()
    );
}

// POST /v0/run with job timeout exceeding free max — clamped to 900
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_timeout_clamped() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob_clamp@example.com").await;
    let org = server.create_org(&user, "Clamp Org").await;
    let project = server.create_project(&user, &org, "Clamp Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Request a 1200s timeout — the org is claimed (has members) with no billing plan,
    // so PlanKind::None applies FREE_MAX (900s) and the timeout should be clamped
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1"),
            "timeout": 1200
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert_eq!(jobs.len(), 1);

    // Verify timeout was clamped to free max (900s)
    {
        use bencher_schema::schema;
        use diesel::{QueryDsl as _, RunQueryDsl as _};
        let mut conn = server.db_conn();
        let stored_timeout: bencher_json::Timeout = schema::job::table
            .select(schema::job::timeout)
            .first(&mut conn)
            .expect("Failed to query job timeout");
        assert_eq!(
            stored_timeout,
            bencher_json::Timeout::try_from(900).unwrap(),
            "Timeout should be clamped to free max"
        );
    }
}

// POST /v0/run with job but no authentication on a claimed project — fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_unauthenticated_with_job_fails() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob_unauth@example.com").await;
    let org = server.create_org(&user, "Unauth Job Org").await;
    let project = server
        .create_project(&user, &org, "Unauth Job Project")
        .await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run with job but NO Authorization header
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
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Organization is claimed (has members), so unauthenticated requests are rejected
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN,
        "Expected auth error, got: {}",
        resp.status()
    );

    // Verify no jobs were created
    let jobs = list_project_jobs(&server, &user, project_slug).await;
    assert!(jobs.is_empty(), "Expected no jobs, got {}", jobs.len());
}

// POST /v0/run with job referencing an archived spec — fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_archived_spec_fails() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_archived@example.com")
        .await;
    let org = server.create_org(&user, "Archived Spec Org").await;
    let project = server
        .create_project(&user, &org, "Archived Spec Project")
        .await;

    // Create a spec then archive it
    let spec = create_spec(&server, &user, "Archived Spec").await;
    let body = serde_json::json!({"archived": true});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", spec.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Failed to archive spec");
    assert_eq!(resp.status(), StatusCode::OK);

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run with job referencing the archived spec
    let archived_slug: &str = spec.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v1"),
            "spec": archived_slug
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // from_active_resource_id filters archived specs, so this should 404
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/run with job and no timeout field — defaults to FREE_MAX for claimed/free org
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_default_timeout() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_deftimeout@example.com")
        .await;
    let org = server.create_org(&user, "Def Timeout Org").await;
    let project = server
        .create_project(&user, &org, "Def Timeout Project")
        .await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    push_test_image(&server, &project, &user, "v1").await;

    // Submit run with job but no timeout field
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify timeout defaults to FREE_MAX (900s) for a claimed org with no plan
    {
        use bencher_schema::schema;
        use diesel::{QueryDsl as _, RunQueryDsl as _};
        let mut conn = server.db_conn();
        let stored_timeout: bencher_json::Timeout = schema::job::table
            .select(schema::job::timeout)
            .first(&mut conn)
            .expect("Failed to query job timeout");
        assert_eq!(
            stored_timeout,
            bencher_json::Timeout::try_from(900).unwrap(),
            "Default timeout should be FREE_MAX (900s)"
        );
    }
}

// POST /v0/run with job — verify priority is Free for a claimed org with no billing plan
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_priority_free() {
    let server = TestServer::new().await;
    let user = server
        .signup("Job User", "runjob_priority@example.com")
        .await;
    let org = server.create_org(&user, "Priority Org").await;
    let project = server.create_project(&user, &org, "Priority Project").await;

    create_fallback_spec(&server, &user).await;

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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify priority is Free (org is claimed, no billing plan)
    {
        use bencher_schema::schema;
        use diesel::{QueryDsl as _, RunQueryDsl as _};
        let mut conn = server.db_conn();
        let stored_priority: bencher_json::JobPriority = schema::job::table
            .select(schema::job::priority)
            .first(&mut conn)
            .expect("Failed to query job priority");
        assert_eq!(
            stored_priority,
            bencher_json::JobPriority::Free,
            "Priority should be Free for a claimed org with no billing plan"
        );
    }
}

// POST /v0/run with job config fields (entrypoint, cmd, env) — verify round-trip
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_config_fields() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob_config@example.com").await;
    let org = server.create_org(&user, "Config Org").await;
    let project = server.create_project(&user, &org, "Config Project").await;

    create_fallback_spec(&server, &user).await;

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
            "image": format!("localhost/{project_slug}:v1"),
            "entrypoint": ["/bin/sh", "-c"],
            "cmd": ["echo", "hello"],
            "env": {"MY_VAR": "my_value", "OTHER": "123"}
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Query config from DB and verify entrypoint, cmd, env round-trip
    {
        use bencher_schema::schema;
        use diesel::{QueryDsl as _, RunQueryDsl as _};
        let mut conn = server.db_conn();
        let stored_config: String = schema::job::table
            .select(schema::job::config)
            .first(&mut conn)
            .expect("Failed to query job config");
        let config: bencher_json::JsonJobConfig =
            serde_json::from_str(&stored_config).expect("Failed to parse job config");

        assert_eq!(
            config.entrypoint.as_deref(),
            Some(["/bin/sh".to_owned(), "-c".to_owned()].as_slice()),
            "Entrypoint should round-trip"
        );
        assert_eq!(
            config.cmd.as_deref(),
            Some(["echo".to_owned(), "hello".to_owned()].as_slice()),
            "Cmd should round-trip"
        );
        let env = config.env.as_ref().expect("env should be present");
        assert_eq!(env.get("MY_VAR").map(String::as_str), Some("my_value"));
        assert_eq!(env.get("OTHER").map(String::as_str), Some("123"));
    }
}

// POST /v0/run with job referencing a nonexistent tag — fails
#[cfg(feature = "plus")]
#[tokio::test]
async fn run_post_with_job_nonexistent_tag_fails() {
    let server = TestServer::new().await;
    let user = server.signup("Job User", "runjob_notag@example.com").await;
    let org = server.create_org(&user, "No Tag Org").await;
    let project = server.create_project(&user, &org, "No Tag Project").await;

    create_fallback_spec(&server, &user).await;

    let project_slug: &str = project.slug.as_ref();
    // Do NOT push any image — reference a tag that does not exist
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results().to_string()],
        "job": {
            "image": format!("localhost/{project_slug}:v999")
        }
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // OCI digest resolution should fail for nonexistent tag
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
