//! Build script for `bencher_runner`.
//!
//! Bundles the `bencher-init` binary for distribution as a single binary.
//!
//! In release builds, the init binary is embedded via `include_bytes!`.
//! In debug builds, it's loaded from disk at runtime for faster iteration.
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

#![expect(clippy::expect_used, clippy::print_stderr, clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Only bundle on Linux where we actually use the init binary
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "linux" {
        generate_stub_module();
        return;
    }

    // Check if plus feature is enabled
    let plus_enabled = env::var("CARGO_FEATURE_PLUS").is_ok();
    if !plus_enabled {
        generate_stub_module();
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let is_release = env::var("PROFILE").unwrap_or_default() == "release";

    // Find the bencher-init binary
    let init_path = find_init_binary();

    if let Some(path) = init_path {
        generate_init_module(&path, is_release, &out_dir);
    } else if is_release {
        // In release builds, we need the binary
        eprintln!("WARNING: bencher-init binary not found for release build.");
        eprintln!("Build it first with: cargo build --release -p bencher_init");
        generate_fallback_module(&out_dir);
    } else {
        // In debug builds, we can fall back to runtime lookup
        generate_fallback_module(&out_dir);
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BENCHER_INIT_PATH");
    println!("cargo:rerun-if-env-changed=PROFILE");
}

/// Find the bencher-init binary.
fn find_init_binary() -> Option<PathBuf> {
    // 1. Check explicit env var
    if let Ok(path) = env::var("BENCHER_INIT_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            eprintln!("Using bencher-init from BENCHER_INIT_PATH: {}", path.display());
            return Some(path);
        }
    }

    // 2. Check target directory (for workspace builds)
    // Look for the binary in the same target profile directory
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Try to find workspace root by looking for Cargo.lock
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

        // Check various possible locations
        // Prefer musl target (statically linked) over native/gnu (dynamically linked)
        let candidates = [
            // With musl target (preferred - statically linked)
            workspace_root.join("target")
                .join(format!("{target_arch}-unknown-linux-musl"))
                .join(&profile)
                .join("bencher-init"),
            // Same profile, no target triple (native build)
            workspace_root.join("target").join(&profile).join("bencher-init"),
            // With gnu target
            workspace_root.join("target")
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

/// Generate the init module with embedded binary.
fn generate_init_module(init_path: &Path, is_release: bool, out_dir: &Path) {
    let module_path = out_dir.join("init_generated.rs");
    let init_path_str = init_path.display();

    let code = if is_release {
        format!(
            r#"// Generated init module - release build with embedded binary.

/// The embedded bencher-init binary.
static INIT_BYTES: &[u8] = include_bytes!("{init_path_str}");

/// Get the bencher-init binary bytes.
///
/// In release builds, the binary is embedded in bencher-runner.
#[inline]
pub fn init_bytes() -> &'static [u8] {{
    INIT_BYTES
}}

/// Whether the init binary is bundled.
pub const INIT_BUNDLED: bool = true;
"#
        )
    } else {
        format!(
            r#"// Generated init module - debug build with runtime loading.

use std::sync::OnceLock;

/// Path to the cached init binary.
const INIT_PATH: &str = "{init_path_str}";

/// Cached init bytes (loaded once on first access).
static INIT_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Get the bencher-init binary bytes.
///
/// In debug builds, the binary is loaded from disk on first access.
pub fn init_bytes() -> &'static [u8] {{
    INIT_BYTES.get_or_init(|| {{
        std::fs::read(INIT_PATH)
            .unwrap_or_else(|e| panic!("Failed to load init from {{}}: {{}}", INIT_PATH, e))
    }})
}}

/// Whether the init binary is bundled.
pub const INIT_BUNDLED: bool = true;
"#
        )
    };

    fs::write(&module_path, code).expect("Failed to write init module");
    eprintln!(
        "Generated init module ({} build) at {}",
        if is_release { "release" } else { "debug" },
        module_path.display()
    );
}

/// Generate a fallback module that looks up the binary at runtime.
fn generate_fallback_module(out_dir: &Path) {
    let module_path = out_dir.join("init_generated.rs");

    let code = r#"// Generated init module - fallback with runtime lookup.

/// Get the bencher-init binary bytes.
///
/// This fallback implementation panics - the binary must be found at runtime.
#[expect(clippy::panic)]
pub fn init_bytes() -> &'static [u8] {
    panic!("bencher-init not bundled. Use find_init_binary() for runtime lookup.")
}

/// Whether the init binary is bundled.
pub const INIT_BUNDLED: bool = false;
"#;

    fs::write(&module_path, code).expect("Failed to write init fallback module");
    eprintln!("Generated init fallback module at {}", module_path.display());
}

/// Generate a stub module for non-Linux platforms.
fn generate_stub_module() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let module_path = out_dir.join("init_generated.rs");

    let code = r#"// Generated init module - stub for non-Linux platforms.

/// Get the bencher-init binary bytes.
///
/// On non-Linux platforms, this panics as the runner is not supported.
#[expect(clippy::panic)]
pub fn init_bytes() -> &'static [u8] {
    panic!("bencher-init not available on this platform - runner requires Linux")
}

/// Whether the init binary is bundled.
pub const INIT_BUNDLED: bool = false;
"#;

    fs::write(&module_path, code).expect("Failed to write init stub module");
}
