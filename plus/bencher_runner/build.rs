//! Build script for `bencher_runner`.
//!
//! Bundles the `bencher-init`, `firecracker`, and `vmlinux` binaries
//! for distribution as a single binary.
//!
//! In release builds, binaries are embedded via `include_bytes!`.
//! In debug builds, they are downloaded/cached locally and loaded from disk at runtime.
//!
//! # Build Process
//!
//! For release builds, first build `bencher-init` for the target:
//! ```sh
//! cargo build --release --target x86_64-unknown-linux-musl -p bencher_init
//! ```
//!
//! Then build `bencher-runner`:
//! ```sh
//! cargo build --release -p bencher_runner --features plus
//! ```
//!
//! # Environment Variable Overrides
//!
//! - `BENCHER_INIT_PATH` — path to a pre-built bencher-init binary
//! - `BENCHER_FIRECRACKER_PATH` — path to a pre-built firecracker binary
//! - `BENCHER_KERNEL_PATH` — path to a pre-built vmlinux kernel

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unreachable,
    clippy::unwrap_used
)]

use std::env;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

/// Default Firecracker version to download.
const DEFAULT_FIRECRACKER_VERSION: &str = "v1.12.0";

/// Default kernel URL to download (per-architecture).
///
/// Uses versioned CI build artifacts from the Firecracker project.
const DEFAULT_KERNEL_URL_X86_64: &str = "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/x86_64/vmlinux-5.10.245";
const DEFAULT_KERNEL_URL_AARCH64: &str = "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20260130-7073e31a0ed7-0/aarch64/vmlinux-5.10.245";

/// Expected SHA256 hashes for Firecracker `.tgz` archives.
///
/// These MUST be updated whenever `DEFAULT_FIRECRACKER_VERSION` changes.
/// Source: `firecracker-{version}-{arch}.tgz.sha256.txt` from the GitHub release.
const FIRECRACKER_TGZ_SHA256_X86_64: &str =
    "392b5f7e4bf12871d1e8377a60ed3b384a46bc2f7d3771caf202aa7a63e32676";
const FIRECRACKER_TGZ_SHA256_AARCH64: &str =
    "55f3e76c6a16128e91aea1d2ed3d436f5d4e2e9547bfdd226ce570a89cd48921";

/// Expected SHA256 hashes for vmlinux kernel binaries.
///
/// These MUST be updated whenever `DEFAULT_KERNEL_URL_*` changes.
/// Compute with: `curl -sL <url> | sha256sum`
const KERNEL_SHA256_X86_64: &str =
    "02cd0f40652b023614b87638de2e5cc53655b88ba82e57eab642adc5a9384c30";
const KERNEL_SHA256_AARCH64: &str =
    "cd752664f99ffa2e460d0fd55846b2394f06a6fce28c7d1d497c8815844a57ff";

fn main() {
    // Only bundle on Linux where we actually use the binaries
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "linux" {
        generate_stub_modules();
        return;
    }

    // Check if plus feature is enabled
    let plus_enabled = env::var("CARGO_FEATURE_PLUS").is_ok();
    if !plus_enabled {
        generate_stub_modules();
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let is_release = env::var("PROFILE").unwrap_or_default() == "release";

    // --- bencher-init ---
    let init_path = find_init_binary();
    if is_release {
        let init_path = init_path.unwrap_or_else(|| panic!("bencher-init binary not found. Build it first with: cargo build --release --target x86_64-unknown-linux-musl -p bencher_init\nOr set BENCHER_INIT_PATH to a pre-built binary.\nThe init binary MUST be statically linked (musl) to run inside the Firecracker VM."));
        generate_binary_module("init", &init_path, is_release, &out_dir);
    } else if let Some(init_path) = init_path {
        generate_binary_module("init", &init_path, is_release, &out_dir);
    } else {
        eprintln!("WARNING: bencher-init not found, generating stub module for debug build");
        generate_stub_module("init", &out_dir);
    }

    // --- firecracker ---
    let firecracker_path = find_or_download_firecracker(&out_dir);
    if is_release {
        let firecracker_path = firecracker_path.unwrap_or_else(|| panic!("firecracker binary not found. Set BENCHER_FIRECRACKER_PATH or ensure download succeeds."));
        generate_binary_module("firecracker", &firecracker_path, is_release, &out_dir);
    } else if let Some(firecracker_path) = firecracker_path {
        generate_binary_module("firecracker", &firecracker_path, is_release, &out_dir);
    } else {
        eprintln!("WARNING: firecracker not found, generating stub module for debug build");
        generate_stub_module("firecracker", &out_dir);
    }

    // --- kernel (vmlinux) ---
    let kernel_path = find_or_download_kernel(&out_dir);
    if is_release {
        let kernel_path = kernel_path.unwrap_or_else(|| {
            panic!("vmlinux kernel not found. Set BENCHER_KERNEL_PATH or ensure download succeeds.")
        });
        generate_binary_module("kernel", &kernel_path, is_release, &out_dir);
    } else if let Some(kernel_path) = kernel_path {
        generate_binary_module("kernel", &kernel_path, is_release, &out_dir);
    } else {
        eprintln!("WARNING: vmlinux kernel not found, generating stub module for debug build");
        generate_stub_module("kernel", &out_dir);
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BENCHER_INIT_PATH");
    println!("cargo:rerun-if-env-changed=BENCHER_FIRECRACKER_PATH");
    println!("cargo:rerun-if-env-changed=BENCHER_KERNEL_PATH");
    println!("cargo:rerun-if-env-changed=PROFILE");
}

// ---------------------------------------------------------------------------
// Binary finders
// ---------------------------------------------------------------------------

/// Find the bencher-init binary.
fn find_init_binary() -> Option<PathBuf> {
    // 1. Check explicit env var
    if let Ok(path) = env::var("BENCHER_INIT_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            eprintln!(
                "Using bencher-init from BENCHER_INIT_PATH: {}",
                path.display()
            );
            return Some(path);
        }
    }

    // 2. Check target directory (for workspace builds)
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = PathBuf::from(&manifest_dir);

        // Go up to find the workspace root
        let mut workspace_root = manifest_path.clone();
        while workspace_root.parent().is_some() {
            if workspace_root.join("Cargo.lock").exists() {
                break;
            }
            workspace_root = workspace_root.parent().unwrap().to_path_buf();
        }

        // Prefer musl target (statically linked) over native/gnu (dynamically linked)
        let candidates = [
            workspace_root
                .join("target")
                .join(format!("{target_arch}-unknown-linux-musl"))
                .join(&profile)
                .join("bencher-init"),
            workspace_root
                .join("target")
                .join(&profile)
                .join("bencher-init"),
            workspace_root
                .join("target")
                .join(format!("{target_arch}-unknown-linux-gnu"))
                .join(&profile)
                .join("bencher-init"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                eprintln!("Found bencher-init at: {}", candidate.display());
                return Some(candidate);
            }
        }
    }

    eprintln!("bencher-init binary not found");
    None
}

/// Find or download the Firecracker binary.
///
/// Checks `BENCHER_FIRECRACKER_PATH` env var first, then tries to download
/// the `.tgz` release archive from GitHub and extract the binary to `OUT_DIR`.
fn find_or_download_firecracker(out_dir: &Path) -> Option<PathBuf> {
    // 1. Check explicit env var
    if let Ok(path) = env::var("BENCHER_FIRECRACKER_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            eprintln!(
                "Using firecracker from BENCHER_FIRECRACKER_PATH: {}",
                path.display()
            );
            return Some(path);
        }
        eprintln!(
            "WARNING: BENCHER_FIRECRACKER_PATH set but file not found: {}",
            path.display()
        );
    }

    // 2. Download from GitHub releases
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let arch = match target_arch.as_str() {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => {
            eprintln!("Unsupported architecture for firecracker: {target_arch}");
            return None;
        },
    };

    let dest = out_dir.join("firecracker");
    if dest.exists() {
        eprintln!("Using cached firecracker at: {}", dest.display());
        return Some(dest);
    }

    let url = format!(
        "https://github.com/firecracker-microvm/firecracker/releases/download/{DEFAULT_FIRECRACKER_VERSION}/firecracker-{DEFAULT_FIRECRACKER_VERSION}-{arch}.tgz",
    );

    // The binary inside the tgz is at:
    // release-{version}-{arch}/firecracker-{version}-{arch}
    let entry_name = format!(
        "release-{DEFAULT_FIRECRACKER_VERSION}-{arch}/firecracker-{DEFAULT_FIRECRACKER_VERSION}-{arch}",
    );

    let expected_hash = match arch {
        "x86_64" => FIRECRACKER_TGZ_SHA256_X86_64,
        "aarch64" => FIRECRACKER_TGZ_SHA256_AARCH64,
        _ => unreachable!(),
    };

    eprintln!("Downloading firecracker from: {url}");
    match download_and_extract_tgz(&url, &entry_name, &dest, Some(expected_hash)) {
        Ok(()) => {
            eprintln!("Extracted firecracker to: {}", dest.display());
            Some(dest)
        },
        Err(e) => {
            eprintln!("WARNING: Failed to download/extract firecracker: {e}");
            None
        },
    }
}

/// Find or download the vmlinux kernel.
///
/// Checks `BENCHER_KERNEL_PATH` env var first, then tries to download
/// from AWS S3 to `OUT_DIR`.
fn find_or_download_kernel(out_dir: &Path) -> Option<PathBuf> {
    // 1. Check explicit env var
    if let Ok(path) = env::var("BENCHER_KERNEL_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            eprintln!("Using kernel from BENCHER_KERNEL_PATH: {}", path.display());
            return Some(path);
        }
        eprintln!(
            "WARNING: BENCHER_KERNEL_PATH set but file not found: {}",
            path.display()
        );
    }

    // 2. Download from S3
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let kernel_url = match target_arch.as_str() {
        "x86_64" => DEFAULT_KERNEL_URL_X86_64,
        "aarch64" => DEFAULT_KERNEL_URL_AARCH64,
        _ => {
            eprintln!("Unsupported architecture for kernel: {target_arch}");
            return None;
        },
    };

    let dest = out_dir.join("vmlinux");
    if dest.exists() {
        eprintln!("Using cached vmlinux at: {}", dest.display());
        return Some(dest);
    }

    let expected_hash = match target_arch.as_str() {
        "x86_64" => KERNEL_SHA256_X86_64,
        "aarch64" => KERNEL_SHA256_AARCH64,
        _ => unreachable!(),
    };

    eprintln!("Downloading vmlinux kernel from: {kernel_url}");
    match download_file(kernel_url, &dest, Some(expected_hash)) {
        Ok(()) => {
            eprintln!("Downloaded vmlinux to: {}", dest.display());
            Some(dest)
        },
        Err(e) => {
            eprintln!("WARNING: Failed to download vmlinux kernel: {e}");
            None
        },
    }
}

/// Download a file from `url` to `dest` using ureq (sync HTTP).
///
/// If `expected_sha256` is provided, the downloaded content is verified before writing.
fn download_file(url: &str, dest: &Path, expected_sha256: Option<&str>) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let mut reader = response.into_body().into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if let Some(expected) = expected_sha256 {
        let hash = Sha256::digest(&bytes);
        let actual = format!("{hash:x}");
        if actual != expected {
            return Err(format!(
                "SHA256 mismatch: expected {expected}, got {actual}"
            ));
        }
        eprintln!("SHA256 verified: {expected}");
    }

    fs::write(dest, &bytes).map_err(|e| format!("Failed to write to {}: {e}", dest.display()))?;

    Ok(())
}

/// Download a `.tgz` archive and extract a single file from it.
///
/// # Arguments
///
/// * `url` - URL of the `.tgz` archive
/// * `entry_name` - Path of the entry to extract (e.g., `release-v1.12.0-x86_64/firecracker-v1.12.0-x86_64`)
/// * `dest` - Destination path for the extracted file
/// * `expected_sha256` - If `Some`, verify the archive's SHA256 before extracting
fn download_and_extract_tgz(
    url: &str,
    entry_name: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    // Read entire archive into memory for hash verification
    let mut archive_bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut archive_bytes)
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    // Verify SHA256 if expected hash is provided
    if let Some(expected) = expected_sha256 {
        let hash = Sha256::digest(&archive_bytes);
        let actual = format!("{hash:x}");
        if actual != expected {
            return Err(format!(
                "SHA256 mismatch for archive: expected {expected}, got {actual}"
            ));
        }
        eprintln!("SHA256 verified: {expected}");
    }

    let gz = flate2::read::GzDecoder::new(archive_bytes.as_slice());
    let mut archive = tar::Archive::new(gz);

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read entry path: {e}"))?;

        if path.to_string_lossy() == entry_name {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Failed to read entry data: {e}"))?;
            fs::write(dest, &bytes)
                .map_err(|e| format!("Failed to write to {}: {e}", dest.display()))?;
            return Ok(());
        }
    }

    Err(format!("Entry '{entry_name}' not found in archive"))
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

/// Generate a module that provides access to an embedded or cached binary.
///
/// - In **release** builds: binary is embedded via `include_bytes!`.
/// - In **debug** builds: binary is loaded from disk via `OnceLock`.
fn generate_binary_module(name: &str, bin_path: &Path, is_release: bool, out_dir: &Path) {
    let module_path = out_dir.join(format!("{name}_generated.rs"));
    let bin_path_str = bin_path.display();
    let name_upper = name.to_uppercase();

    let code = if is_release {
        format!(
            r#"// Generated {name} module - release build with embedded binary.

/// The embedded {name} binary.
static {name_upper}_BYTES: &[u8] = include_bytes!("{bin_path_str}");

/// Get the {name} binary bytes.
///
/// In release builds, the binary is embedded in bencher-runner.
#[inline]
pub fn {name}_bytes() -> &'static [u8] {{
    {name_upper}_BYTES
}}

/// Whether the {name} binary is bundled.
pub const {name_upper}_BUNDLED: bool = true;
"#
        )
    } else {
        format!(
            r#"// Generated {name} module - debug build with runtime loading.

use std::sync::OnceLock;

/// Path to the cached {name} binary.
const {name_upper}_PATH: &str = "{bin_path_str}";

/// Cached {name} bytes (loaded once on first access).
static {name_upper}_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Get the {name} binary bytes.
///
/// In debug builds, the binary is loaded from disk on first access.
pub fn {name}_bytes() -> &'static [u8] {{
    {name_upper}_BYTES.get_or_init(|| {{
        std::fs::read({name_upper}_PATH)
            .unwrap_or_else(|e| panic!("Failed to load {name} from {{}}: {{}}", {name_upper}_PATH, e))
    }})
}}

/// Whether the {name} binary is bundled.
pub const {name_upper}_BUNDLED: bool = true;
"#
        )
    };

    fs::write(&module_path, code).unwrap_or_else(|_| panic!("Failed to write {name} module"));
    eprintln!(
        "Generated {name} module ({} build) at {}",
        if is_release { "release" } else { "debug" },
        module_path.display()
    );
}

/// Generate stub modules for non-Linux platforms or when plus is disabled.
fn generate_stub_modules() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    for name in &["init", "firecracker", "kernel"] {
        generate_stub_module(name, &out_dir);
    }
}

/// Generate a single stub module for a binary that is not available.
fn generate_stub_module(name: &str, out_dir: &Path) {
    let module_path = out_dir.join(format!("{name}_generated.rs"));
    let name_upper = name.to_uppercase();

    let code = format!(
        r#"// Generated {name} module - stub (binary not available).

/// Get the {name} binary bytes.
///
/// This is a stub — the {name} binary was not available at build time.
#[expect(clippy::panic)]
pub fn {name}_bytes() -> &'static [u8] {{
    panic!("{name} binary not available - build it first or set the corresponding env var")
}}

/// Whether the {name} binary is bundled.
pub const {name_upper}_BUNDLED: bool = false;
"#
    );

    fs::write(&module_path, code).unwrap_or_else(|_| panic!("Failed to write {name} stub module"));
}
