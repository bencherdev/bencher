//! Bundled vmlinux kernel support.
//!
//! This module provides access to the bundled vmlinux kernel image.
//!
//! - In **release** builds: The kernel is embedded directly in bencher-runner.
//! - In **debug** builds: The kernel is loaded from disk (downloaded by build.rs).
//!
//! # Example
//!
//! ```ignore
//! use bencher_runner::kernel::{write_kernel_to_file, KERNEL_BUNDLED};
//!
//! if KERNEL_BUNDLED {
//!     write_kernel_to_file("/tmp/vmlinux".as_ref())?;
//! }
//! ```

use std::io;

use camino::Utf8Path;

// Include the generated kernel module
include!(concat!(env!("OUT_DIR"), "/kernel_generated.rs"));

/// Write the bundled vmlinux kernel to a file.
///
/// # Arguments
///
/// * `path` - The destination path for the kernel image
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_kernel_to_file(path: &Utf8Path) -> io::Result<()> {
    std::fs::write(path, kernel_bytes())?;
    Ok(())
}
