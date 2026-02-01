//! Bundled kernel support.
//!
//! This module provides access to the bundled Linux kernel for booting VMs.
//! The kernel is downloaded at build time from Firecracker's CI artifacts.
//!
//! - In **release** builds: The kernel is embedded directly in the binary.
//! - In **debug** builds: The kernel is loaded from disk to speed up compilation.
//!
//! # Example
//!
//! ```ignore
//! use bencher_vmm::kernel_bytes;
//! use std::fs;
//!
//! // Write the bundled kernel to a file for use with KVM
//! let kernel_path = "/tmp/vmlinux";
//! fs::write(kernel_path, kernel_bytes())?;
//! ```

use std::io;
use std::path::Path;

// Include the generated kernel module
include!(concat!(env!("OUT_DIR"), "/kernel_generated.rs"));

/// Write the bundled kernel to a file.
///
/// This is a convenience function for writing the kernel bytes to disk,
/// which is required because KVM expects a file path rather than raw bytes.
///
/// # Arguments
///
/// * `path` - The destination path for the kernel file
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_kernel_to_file(path: &Path) -> io::Result<()> {
    std::fs::write(path, kernel_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_bytes_not_empty() {
        let bytes = kernel_bytes();
        assert!(!bytes.is_empty(), "Kernel bytes should not be empty");
        // Linux kernels are typically at least a few MB
        assert!(
            bytes.len() > 1_000_000,
            "Kernel should be larger than 1MB, got {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn test_kernel_has_elf_magic() {
        let bytes = kernel_bytes();
        // ELF magic number: 0x7f 'E' 'L' 'F'
        assert!(
            bytes.len() >= 4,
            "Kernel too small to contain ELF header"
        );
        assert_eq!(
            &bytes[0..4],
            &[0x7f, b'E', b'L', b'F'],
            "Kernel should start with ELF magic number"
        );
    }
}
