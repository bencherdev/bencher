//! Bundled Firecracker binary support.
//!
//! This module provides access to the bundled Firecracker binary.
//!
//! - In **release** builds: The binary is embedded directly in bencher-runner.
//! - In **debug** builds: The binary is loaded from disk (downloaded by build.rs).
//!
//! # Example
//!
//! ```ignore
//! use bencher_runner::firecracker_bin::{write_firecracker_to_file, FIRECRACKER_BUNDLED};
//!
//! if FIRECRACKER_BUNDLED {
//!     write_firecracker_to_file("/tmp/firecracker".as_ref())?;
//! }
//! ```

use std::io;

use camino::Utf8Path;

// Include the generated firecracker module
include!(concat!(env!("OUT_DIR"), "/firecracker_generated.rs"));

/// Write the bundled Firecracker binary to a file.
///
/// The file is written with executable permissions (0o755).
///
/// # Arguments
///
/// * `path` - The destination path for the Firecracker binary
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_firecracker_to_file(path: &Utf8Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, firecracker_bytes())?;

    // Make it executable
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;

    Ok(())
}
