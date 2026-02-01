use std::fs::File;
use std::io::Write as _;

use anyhow::Context as _;
use camino::Utf8PathBuf;

use crate::parser::TaskKernel;

/// URL for a Firecracker-compatible kernel from CI.
///
/// Uses versioned CI build artifacts for reproducibility.
/// Format: firecracker-ci/{build-id}/{arch}/vmlinux-{version}
#[cfg(target_arch = "x86_64")]
const KERNEL_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/x86_64/vmlinux-5.10.245";
#[cfg(target_arch = "aarch64")]
const KERNEL_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/aarch64/vmlinux-5.10.245";
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const KERNEL_URL: &str = "";

#[derive(Debug)]
pub struct Kernel {}

impl TryFrom<TaskKernel> for Kernel {
    type Error = anyhow::Error;

    fn try_from(_kernel: TaskKernel) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Kernel {
    #[expect(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        ensure_kernel()
    }
}

/// Get the path to the kernel image.
pub fn kernel_path() -> Utf8PathBuf {
    super::work_dir().join("vmlinux")
}

/// Ensure the kernel is downloaded and available.
pub fn ensure_kernel() -> anyhow::Result<()> {
    let path = kernel_path();

    if path.exists() {
        println!("Kernel already exists at {path}");
        return Ok(());
    }

    println!("Downloading kernel from {KERNEL_URL}...");

    let response = reqwest::blocking::get(KERNEL_URL).context("Failed to download kernel")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download kernel: HTTP {}", response.status());
    }

    let bytes = response.bytes().context("Failed to read kernel bytes")?;

    let mut file = File::create(&path).context("Failed to create kernel file")?;
    file.write_all(&bytes).context("Failed to write kernel")?;

    println!("Kernel downloaded to {path}");
    println!("Size: {} bytes", bytes.len());

    Ok(())
}
