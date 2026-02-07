//! Squashfs image creation.
//!
//! This module creates squashfs filesystem images from directories.
//! Squashfs is ideal for read-only rootfs images because it's:
//! - Compressed (smaller image size)
//! - Read-only (perfect for immutable benchmark environments)
//! - Supported by Linux kernel natively
//! - Fast to mount

use std::fs::{self, File};
use std::io::Cursor;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

use backhand::compression::Compressor;
use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};
use camino::Utf8Path;

use crate::error::RootfsError;

/// Default block size for squashfs (128 KiB).
const DEFAULT_BLOCK_SIZE: u32 = 0x2_0000;

/// Create a squashfs image from a directory.
///
/// # Arguments
///
/// * `source_dir` - The directory to pack into squashfs
/// * `output_path` - Path to write the squashfs image
///
/// # Example
///
/// ```ignore
/// use camino::Utf8Path;
/// use bencher_rootfs::create_squashfs;
///
/// create_squashfs(
///     Utf8Path::new("/path/to/rootfs"),
///     Utf8Path::new("/path/to/rootfs.squashfs"),
/// )?;
/// ```
pub fn create_squashfs(source_dir: &Utf8Path, output_path: &Utf8Path) -> Result<(), RootfsError> {
    // Create a new filesystem writer with gzip compression
    let mut writer = FilesystemWriter::default();
    let compressor = FilesystemCompressor::new(Compressor::Gzip, None)?;
    writer.set_compressor(compressor);
    writer.set_block_size(DEFAULT_BLOCK_SIZE);

    // Add all files from the source directory
    add_directory_recursive(&mut writer, source_dir, Utf8Path::new(""))?;

    // Write the squashfs image
    let output_file = File::create(output_path)?;
    writer.write(output_file)?;

    Ok(())
}

/// Recursively add a directory and its contents to the filesystem writer.
#[expect(clippy::filetype_is_file)]
fn add_directory_recursive(
    writer: &mut FilesystemWriter,
    source_dir: &Utf8Path,
    relative_path: &Utf8Path,
) -> Result<(), RootfsError> {
    let full_path = source_dir.join(relative_path);

    for entry in fs::read_dir(&full_path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        let entry_relative = if relative_path.as_str().is_empty() {
            camino::Utf8PathBuf::from(file_name_str.as_ref())
        } else {
            relative_path.join(file_name_str.as_ref())
        };

        let metadata = entry.metadata()?;
        let file_type = metadata.file_type();

        let header = create_node_header(&metadata);

        if file_type.is_dir() {
            // Add directory
            writer
                .push_dir(entry_relative.as_str(), header)
                .map_err(|e| RootfsError::Squashfs(e.to_string()))?;

            // Recurse into directory
            add_directory_recursive(writer, source_dir, &entry_relative)?;
        } else if file_type.is_symlink() {
            // Add symlink (check before is_file since symlinks can also return true for is_file)
            let target = fs::read_link(entry.path())?;
            let target_str = target.to_string_lossy();
            writer
                .push_symlink(target_str.as_ref(), entry_relative.as_str(), header)
                .map_err(|e| RootfsError::Squashfs(e.to_string()))?;
        } else if file_type.is_file() {
            // Add regular file
            let content = fs::read(entry.path())?;
            let reader = Cursor::new(content);
            writer
                .push_file(reader, entry_relative.as_str(), header)
                .map_err(|e| RootfsError::Squashfs(e.to_string()))?;
        }
        // Skip other file types (devices, sockets, etc.)
    }

    Ok(())
}

/// Create a node header from file metadata.
fn create_node_header(metadata: &fs::Metadata) -> NodeHeader {
    NodeHeader {
        permissions: (metadata.permissions().mode() & 0o7777) as u16,
        uid: metadata.uid(),
        gid: metadata.gid(),
        mtime: u32::try_from(metadata.mtime()).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_block_size() {
        assert_eq!(DEFAULT_BLOCK_SIZE, 0x2_0000);
    }
}
