//! OCI Distribution Spec Conformance Test Infrastructure
//!
//! This module provides infrastructure for running the official OCI Distribution
//! Specification conformance tests against the Bencher OCI registry.
//!
//! # Running Conformance Tests
//!
//! 1. Start the Bencher API server:
//!    ```sh
//!    cargo run -p bencher_api --features plus
//!    ```
//!
//! 2. Run the conformance tests:
//!    ```sh
//!    ./plus/bencher_oci/scripts/run_conformance.sh
//!    ```
//!
//! # Test Categories
//!
//! The OCI conformance tests cover four categories:
//! - **Pull**: Required - downloading blobs and manifests
//! - **Push**: Recommended - uploading blobs and manifests
//! - **Content Discovery**: Optional - listing tags
//! - **Content Management**: Optional - deleting content

#![cfg(test)]
#![cfg(feature = "plus")]
// Test files link main crate dependencies even when not directly used
#![allow(unused_crate_dependencies)]

use std::net::TcpStream;

const API_HOST: &str = "localhost";
const API_PORT: u16 = 61016;

/// Check if the API server is running
fn is_api_running() -> bool {
    TcpStream::connect(format!("{API_HOST}:{API_PORT}")).is_ok()
}

/// Basic connectivity test to verify the OCI registry is responding
#[test]
#[ignore = "requires API server to be running"]
fn test_oci_base_endpoint() {
    if !is_api_running() {
        eprintln!("API server not running, skipping test");
        return;
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("http://{API_HOST}:{API_PORT}/v2/"))
        .send()
        .expect("Failed to connect to OCI registry");

    assert_eq!(
        response.status(),
        200,
        "OCI base endpoint should return 200"
    );
}

/// Smoke test for blob upload flow
#[test]
#[ignore = "requires API server to be running"]
fn test_oci_blob_upload_smoke() {
    if !is_api_running() {
        eprintln!("API server not running, skipping test");
        return;
    }

    let client = reqwest::blocking::Client::new();

    // Start upload
    let response = client
        .post(format!(
            "http://{API_HOST}:{API_PORT}/v2/test/repo/blobs/uploads/"
        ))
        .send()
        .expect("Failed to start blob upload");

    assert!(
        response.status().is_success() || response.status().as_u16() == 202,
        "Blob upload start should succeed"
    );

    // Check for Location header
    let location = response
        .headers()
        .get("Location")
        .expect("Response should have Location header");
    assert!(!location.is_empty(), "Location header should not be empty");
}

/// Print instructions for running full conformance tests
#[test]
fn print_conformance_instructions() {
    println!("\n");
    println!("=== OCI Distribution Conformance Testing ===\n");
    println!("To run the full OCI conformance tests:\n");
    println!("1. Install Go 1.17+ if not already installed\n");
    println!("2. Clone and build the conformance tests:");
    println!("   git clone https://github.com/opencontainers/distribution-spec.git");
    println!("   cd distribution-spec/conformance");
    println!("   go test -c\n");
    println!("3. Start the Bencher API server:");
    println!("   cargo run -p bencher_api --features plus\n");
    println!("4. Run the conformance tests:");
    println!("   export OCI_ROOT_URL=http://localhost:61016");
    println!("   export OCI_NAMESPACE=test/repo");
    println!("   export OCI_CROSSMOUNT_NAMESPACE=test/other");
    println!("   export OCI_TEST_PULL=1");
    println!("   export OCI_TEST_PUSH=1");
    println!("   export OCI_TEST_CONTENT_DISCOVERY=1");
    println!("   export OCI_TEST_CONTENT_MANAGEMENT=1");
    println!("   ./conformance.test\n");
    println!("5. Check results in junit.xml and report.html\n");
    println!("=============================================\n");
}
