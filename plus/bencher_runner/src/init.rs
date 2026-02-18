//! Bundled init binary support.
//!
//! This module provides access to the bundled `bencher-init` binary.
//!
//! - In **release** builds: The binary is embedded directly in bencher-runner.
//! - In **debug** builds: The binary is loaded from disk to speed up compilation.
//!
//! # Example
//!
//! ```ignore
//! use bencher_runner::init::{write_init_to_file, INIT_BUNDLED};
//!
//! if INIT_BUNDLED {
//!     write_init_to_file("/path/to/rootfs/init")?;
//! }
//! ```

use std::io;

use camino::Utf8Path;

// Include the generated init module
include!(concat!(env!("OUT_DIR"), "/init_generated.rs"));

/// Write the bundled init binary to a file.
///
/// # Arguments
///
/// * `path` - The destination path for the init binary
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_init_to_file(path: &Utf8Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::write(path, init_bytes())?;

    // Make it executable
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;

    Ok(())
}
