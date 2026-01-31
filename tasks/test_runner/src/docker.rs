//! Docker support for running the test on macOS.
//!
//! On macOS, we don't have KVM, so we run the test inside a Docker container
//! that has access to nested virtualization (Docker Desktop with Rosetta or
//! hardware virtualization).

use std::process::Command;

use anyhow::Context as _;
use camino::Utf8Path;

/// Check if we're running on Linux.
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Check if Docker is available.
pub fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if we can run KVM inside Docker.
pub fn docker_kvm_available() -> bool {
    // Try to run a quick check for /dev/kvm in Docker
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--device=/dev/kvm",
            "alpine:latest",
            "test",
            "-c",
            "/dev/kvm",
        ])
        .output();

    output.map(|o| o.status.success()).unwrap_or(false)
}

/// Run the test inside Docker.
pub fn run_in_docker(workspace_root: &Utf8Path) -> anyhow::Result<String> {
    println!("Running test in Docker...");

    // Check if Docker is available
    if !docker_available() {
        anyhow::bail!("Docker is not available. Please install Docker Desktop.");
    }

    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--device=/dev/kvm",
            "-v",
            &format!("{workspace_root}:/workspace:ro"),
            "-w",
            "/workspace",
            "rust:1.86-bookworm",
            "bash",
            "-c",
            "apt-get update && apt-get install -y qemu-kvm >/dev/null 2>&1 && \
             cd /workspace && \
             cargo test-runner test 2>&1",
        ])
        .output()
        .context("Failed to run docker")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        println!("Docker stdout: {stdout}");
        println!("Docker stderr: {stderr}");
        anyhow::bail!("Docker test failed");
    }

    Ok(stdout)
}
