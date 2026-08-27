//! Bundled Firecracker jailer binary support.
//!
//! This module provides access to the bundled `jailer` binary, which confines
//! the Firecracker VMM to a chroot under an unprivileged uid.
//!
//! - In **release** builds: The binary is embedded directly in bencher-runner.
//! - In **debug** builds: The binary is loaded from disk (downloaded by build.rs).
//!
//! The jailer ships in the same release archive as Firecracker, so both are
//! bundled from a single download under a single hash. Extracting both per job
//! keeps the VMM and its jailer at the same version across a runner self-update.
//!
//! # Example
//!
//! ```ignore
//! use bencher_runner::jailer_bin::{write_jailer_to_file, JAILER_BUNDLED};
//!
//! if JAILER_BUNDLED {
//!     write_jailer_to_file("/tmp/jailer".as_ref())?;
//! }
//! ```

use std::io;

use camino::Utf8Path;

// Include the generated jailer module
include!(concat!(env!("OUT_DIR"), "/jailer_generated.rs"));

/// Write the bundled jailer binary to a file.
///
/// The file is written with executable permissions (0o755).
///
/// # Arguments
///
/// * `path` - The destination path for the jailer binary
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_jailer_to_file(path: &Utf8Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, jailer_bytes())?;

    // Make it executable
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;

    Ok(())
}
