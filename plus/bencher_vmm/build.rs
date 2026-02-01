//! Build script for `bencher_vmm`.
//!
//! Downloads Firecracker-compatible kernels for supported architectures
//! and generates code to access them at runtime.
//!
//! # Security
//!
//! Downloaded kernels are verified against known SHA256 hashes to ensure
//! integrity. If a hash doesn't match, the build will fail.

// Build scripts commonly use expect/panic for error handling and eprintln for output
#![expect(clippy::expect_used)]
#![expect(clippy::panic)]
#![expect(clippy::print_stderr)]

use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::PathBuf;

/// Firecracker CI build identifier for kernel downloads.
///
/// This is the date-based build ID from Firecracker's CI system.
/// Format: YYYYMMDD-<git-commit-hash>-<build-number>
///
/// To find the latest build ID:
/// ```sh
/// curl -s "http://spec.ccfc.min.s3.amazonaws.com/?prefix=firecracker-ci/&list-type=2" | \
///     tr '<' '\n' | grep 'Key>firecracker-ci/202' | grep 'vmlinux-5.10' | \
///     grep -v config | grep -v debug | sort -r | head -1
/// ```
const FIRECRACKER_CI_BUILD: &str = "20260130-7073e31a0ed7-0";

/// Kernel version to use (5.10 or 6.1).
const KERNEL_VERSION: &str = "5.10.245";

/// Base URL for Firecracker CI artifacts.
const BASE_URL: &str = "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci";

/// Kernel configurations for each architecture.
struct KernelConfig {
    arch: &'static str,
    target_arch: &'static str,
    filename: &'static str,
    /// Expected SHA256 hash of the kernel binary (hex-encoded).
    expected_sha256: &'static str,
}

/// SHA256 hashes for Firecracker CI kernels.
///
/// These hashes were computed from the official Firecracker CI artifacts.
/// To verify manually:
/// ```sh
/// curl -sL https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/x86_64/vmlinux-5.10.245 | sha256sum
/// curl -sL https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/aarch64/vmlinux-5.10.245 | sha256sum
/// ```
const KERNELS: &[KernelConfig] = &[
    KernelConfig {
        arch: "x86_64",
        target_arch: "x86_64",
        filename: "vmlinux-x86_64.bin",
        expected_sha256: "02cd0f40652b023614b87638de2e5cc53655b88ba82e57eab642adc5a9384c30",
    },
    KernelConfig {
        arch: "aarch64",
        target_arch: "aarch64",
        filename: "vmlinux-aarch64.bin",
        expected_sha256: "cd752664f99ffa2e460d0fd55846b2394f06a6fce28c7d1d497c8815844a57ff",
    },
];

fn main() {
    // Only run on Linux where we actually use the kernels
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "linux" {
        generate_stub_module();
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let is_release = env::var("PROFILE").unwrap_or_default() == "release";

    // Find the kernel for the target architecture
    let kernel = KERNELS
        .iter()
        .find(|k| k.target_arch == target_arch)
        .expect("Unsupported target architecture");

    let kernel_path = out_dir.join(kernel.filename);

    // Download kernel if not already cached
    if !kernel_path.exists() {
        download_kernel(kernel, &kernel_path);
    }

    // Generate the kernel module
    generate_kernel_module(&kernel_path, is_release);

    // Tell Cargo to rerun if these change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PROFILE");
}

fn download_kernel(kernel: &KernelConfig, dest: &std::path::Path) {
    let url = format!(
        "{BASE_URL}/{FIRECRACKER_CI_BUILD}/{}/vmlinux-{KERNEL_VERSION}",
        kernel.arch
    );

    eprintln!("Downloading kernel from {url}...");

    let mut response = ureq::get(&url)
        .call()
        .unwrap_or_else(|e| panic!("Failed to download kernel from {url}: {e}"));

    assert!(
        response.status() == 200,
        "Failed to download kernel: HTTP {} from {url}",
        response.status(),
    );

    // Read the response body (kernels are ~40MB, so increase limit from default 10MB)
    let bytes = response
        .body_mut()
        .with_config()
        .limit(100 * 1024 * 1024) // 100MB limit
        .read_to_vec()
        .expect("Failed to read kernel bytes");

    // Verify SHA256 hash before writing to disk
    verify_sha256(&bytes, kernel.expected_sha256, &url);

    // Write to destination
    let mut file = File::create(dest).expect("Failed to create kernel file");
    file.write_all(&bytes).expect("Failed to write kernel");

    eprintln!(
        "Downloaded and verified {} ({} bytes) to {}",
        kernel.filename,
        bytes.len(),
        dest.display()
    );
}

/// Verify that the downloaded bytes match the expected SHA256 hash.
fn verify_sha256(bytes: &[u8], expected_hex: &str, source: &str) {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let computed = hasher.finalize();
    let computed_hex = hex_encode(&computed);

    if computed_hex != expected_hex {
        panic!(
            "SHA256 hash mismatch for {source}!\n\
             Expected: {expected_hex}\n\
             Computed: {computed_hex}\n\
             \n\
             This could indicate:\n\
             - The file was corrupted during download\n\
             - The upstream file has changed (update the expected hash)\n\
             - A potential supply chain attack\n\
             \n\
             Please verify the hash manually and update the expected value if legitimate."
        );
    }

    eprintln!("SHA256 verified: {computed_hex}");
}

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn generate_kernel_module(kernel_path: &std::path::Path, is_release: bool) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let module_path = out_dir.join("kernel_generated.rs");

    let kernel_path_str = kernel_path.display();

    let code = if is_release {
        // Release: embed kernel bytes directly in the binary
        format!(
            "// Generated kernel module - release build with embedded kernel.

/// The embedded kernel bytes.
static KERNEL_BYTES: &[u8] = include_bytes!(\"{kernel_path_str}\");

/// Get the kernel bytes.
///
/// In release builds, the kernel is embedded in the binary.
#[inline]
pub fn kernel_bytes() -> &'static [u8] {{
    KERNEL_BYTES
}}
"
        )
    } else {
        // Debug: load kernel from cached file at runtime
        format!(
            "// Generated kernel module - debug build with runtime loading.

use std::sync::OnceLock;

/// Path to the cached kernel file.
const KERNEL_PATH: &str = \"{kernel_path_str}\";

/// Cached kernel bytes (loaded once on first access).
static KERNEL_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Get the kernel bytes.
///
/// In debug builds, the kernel is loaded from disk on first access.
/// This avoids embedding ~40MB in the binary during development.
pub fn kernel_bytes() -> &'static [u8] {{
    KERNEL_BYTES.get_or_init(|| {{
        std::fs::read(KERNEL_PATH)
            .unwrap_or_else(|e| panic!(\"Failed to load kernel from {{}}: {{}}\", KERNEL_PATH, e))
    }})
}}
"
        )
    };

    fs::write(&module_path, code).expect("Failed to write kernel module");

    eprintln!(
        "Generated kernel module ({} build) at {}",
        if is_release { "release" } else { "debug" },
        module_path.display()
    );
}

fn generate_stub_module() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let module_path = out_dir.join("kernel_generated.rs");

    let code = "// Generated kernel module - stub for non-Linux platforms.

/// Get the kernel bytes.
///
/// On non-Linux platforms, this panics as the VMM is not supported.
#[expect(clippy::panic)]
pub fn kernel_bytes() -> &'static [u8] {
    panic!(\"Kernel not available on this platform - VMM requires Linux with KVM\")
}
";

    fs::write(&module_path, code).expect("Failed to write kernel stub module");
}
