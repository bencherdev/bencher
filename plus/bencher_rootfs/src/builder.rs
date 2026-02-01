//! OCI to rootfs builder pipeline.
//!
//! This module provides a high-level interface for converting an OCI image
//! to a bootable squashfs rootfs image.

use camino::Utf8Path;

use crate::error::RootfsError;
use crate::squashfs::create_squashfs;

/// Build a squashfs rootfs from an OCI image.
///
/// This is a convenience function that:
/// 1. Unpacks the OCI image to a temporary directory
/// 2. Creates a squashfs image from the unpacked contents
/// 3. Cleans up the temporary directory
///
/// # Arguments
///
/// * `oci_image_dir` - Path to the OCI image directory
/// * `output_path` - Path to write the squashfs image
///
/// # Example
///
/// ```ignore
/// use camino::Utf8Path;
/// use bencher_rootfs::build_rootfs;
///
/// build_rootfs(
///     Utf8Path::new("/path/to/oci-image"),
///     Utf8Path::new("/path/to/rootfs.squashfs"),
/// )?;
/// ```
pub fn build_rootfs(oci_image_dir: &Utf8Path, output_path: &Utf8Path) -> Result<(), RootfsError> {
    // Create a temporary directory for unpacking
    let temp_dir = tempdir()?;
    let unpack_dir = Utf8Path::from_path(temp_dir.path())
        .ok_or_else(|| RootfsError::Path("Invalid temp directory path".to_owned()))?;

    // Unpack the OCI image
    bencher_oci::unpack(oci_image_dir, unpack_dir)?;

    // Create squashfs from the unpacked directory
    create_squashfs(unpack_dir, output_path)?;

    // Temp directory is automatically cleaned up on drop
    Ok(())
}

/// Build a squashfs rootfs from an already unpacked directory.
///
/// Use this when you've already unpacked the OCI image or have
/// a directory you want to convert directly to squashfs.
///
/// # Arguments
///
/// * `source_dir` - Path to the unpacked rootfs directory
/// * `output_path` - Path to write the squashfs image
pub fn build_rootfs_from_dir(
    source_dir: &Utf8Path,
    output_path: &Utf8Path,
) -> Result<(), RootfsError> {
    create_squashfs(source_dir, output_path)
}

/// Create a temporary directory.
fn tempdir() -> Result<TempDir, RootfsError> {
    let path = std::env::temp_dir().join(format!("bencher-rootfs-{}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(TempDir { path })
}

/// A temporary directory that is deleted when dropped.
struct TempDir {
    path: std::path::PathBuf,
}

impl TempDir {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.path));
    }
}
