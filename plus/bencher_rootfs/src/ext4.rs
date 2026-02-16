//! ext4 image creation.
//!
//! This module creates ext4 filesystem images from directories.
//! ext4 is used for writable rootfs images because it:
//! - Is the standard Linux filesystem
//! - Supports read/write operations
//! - Is supported by Linux kernel natively
//! - Has good performance

use std::fs::File;
use std::process::Command;

use camino::Utf8Path;

use crate::error::RootfsError;

/// Default ext4 image size in MiB.
const DEFAULT_IMAGE_SIZE_MIB: u64 = 1024;

/// Minimum ext4 image size in MiB.
const MIN_IMAGE_SIZE_MIB: u64 = 64;

/// Create an ext4 image from a directory.
///
/// This creates an ext4 filesystem image containing the contents of the
/// source directory. The image is writable, allowing the guest to create and
/// modify files during execution.
///
/// # Arguments
///
/// * `source_dir` - The directory to pack into ext4
/// * `output_path` - Path to write the ext4 image
///
/// # Example
///
/// ```ignore
/// use camino::Utf8Path;
/// use bencher_rootfs::create_ext4;
///
/// create_ext4(
///     Utf8Path::new("/path/to/rootfs"),
///     Utf8Path::new("/path/to/rootfs.ext4"),
/// )?;
/// ```
pub fn create_ext4(source_dir: &Utf8Path, output_path: &Utf8Path) -> Result<(), RootfsError> {
    create_ext4_with_size(source_dir, output_path, DEFAULT_IMAGE_SIZE_MIB)
}

/// Create an ext4 image with a specific size.
///
/// # Arguments
///
/// * `source_dir` - The directory to pack into ext4
/// * `output_path` - Path to write the ext4 image
/// * `size_mib` - Size of the image in MiB
pub fn create_ext4_with_size(
    source_dir: &Utf8Path,
    output_path: &Utf8Path,
    size_mib: u64,
) -> Result<(), RootfsError> {
    let size_mib = size_mib.max(MIN_IMAGE_SIZE_MIB);

    // Step 1: Create a file of the desired size for mkfs.ext4 to format
    create_file(output_path, size_mib)?;

    // Step 2: Format as ext4 and populate with directory contents
    // mkfs.ext4 -d option copies directory contents during creation
    let output = Command::new("mkfs.ext4")
        .args([
            "-F", // Force, even if the file exists
            "-q", // Quiet mode
            "-m",
            "0", // No reserved blocks
            "-d",
            source_dir.as_str(), // Populate from directory
            output_path.as_str(),
        ])
        .output()
        .map_err(|e| RootfsError::Ext4(format!("failed to run mkfs.ext4: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RootfsError::Ext4(format!("mkfs.ext4 failed: {stderr}")));
    }

    // Step 3: Best-effort physical block allocation.
    // mkfs.ext4 creates a sparse file internally. fallocate converts sparse
    // regions to real blocks, bounding the physical size of the disk image.
    // This is defense-in-depth — if the host filesystem doesn't have enough
    // space for full allocation, we proceed with the sparse image.
    drop(allocate_file(output_path, size_mib));

    Ok(())
}

/// Create a file of the specified size using `set_len`.
///
/// This creates a sparse file for `mkfs.ext4` to format.
fn create_file(path: &Utf8Path, size_mib: u64) -> Result<(), RootfsError> {
    let file = File::create(path)?;
    let size_bytes = size_mib * 1024 * 1024;
    file.set_len(size_bytes)?;
    Ok(())
}

/// Ensure all blocks in a file are physically allocated.
///
/// On Linux, uses `fallocate` to convert sparse regions to real blocks,
/// preventing the guest from writing beyond the intended disk limit.
/// Must be called *after* `mkfs.ext4` since mkfs internally creates sparse regions.
///
/// On other platforms, this is a no-op (ext4 creation only works on Linux anyway).
#[cfg(target_os = "linux")]
fn allocate_file(path: &Utf8Path, size_mib: u64) -> Result<(), RootfsError> {
    use std::os::fd::AsRawFd;

    use nix::fcntl::{FallocateFlags, fallocate};

    let file = File::options().write(true).open(path)?;
    let size_bytes = size_mib * 1024 * 1024;
    #[expect(clippy::cast_possible_wrap, reason = "Practical disk sizes fit in i64")]
    let size_i64 = size_bytes as i64;
    fallocate(file.as_raw_fd(), FallocateFlags::empty(), 0, size_i64)
        .map_err(|e| RootfsError::Ext4(format!("fallocate failed: {e}")))?;
    Ok(())
}

/// No-op on non-Linux (ext4 creation only works on Linux).
#[cfg(not(target_os = "linux"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Signature must match the Linux variant"
)]
fn allocate_file(_path: &Utf8Path, _size_mib: u64) -> Result<(), RootfsError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn create_file_correct_size() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = Utf8Path::from_path(temp_dir.path())
            .unwrap()
            .join("test.img");

        create_file(&image_path, 64).unwrap();

        let metadata = fs::metadata(&image_path).unwrap();
        assert_eq!(metadata.len(), 64 * 1024 * 1024);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn allocate_file_not_sparse() {
        use std::os::unix::fs::MetadataExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = Utf8Path::from_path(temp_dir.path())
            .unwrap()
            .join("test.img");

        // Create first, then allocate (mirrors real usage)
        create_file(&image_path, 64).unwrap();
        allocate_file(&image_path, 64).unwrap();

        let metadata = fs::metadata(&image_path).unwrap();
        // fallocate pre-allocates physical blocks, so blocks should be > 0
        assert!(
            metadata.blocks() > 0,
            "Expected physical blocks to be allocated, got 0"
        );
        // Verify substantial allocation (64 MiB = 131072 512-byte blocks)
        assert!(
            metadata.blocks() >= 131072,
            "Expected at least 131072 blocks for 64 MiB, got {}",
            metadata.blocks()
        );
    }
}
