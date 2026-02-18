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

        let metadata = entry.path().symlink_metadata()?;
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

    use std::io::BufReader;
    use std::os::unix::fs as unix_fs;

    use backhand::{FilesystemReader, InnerNode};

    #[test]
    fn default_block_size() {
        assert_eq!(DEFAULT_BLOCK_SIZE, 0x2_0000);
    }

    #[test]
    fn create_squashfs_preserves_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(&source).unwrap();

        // Create a regular file and a symlink pointing to it
        fs::write(source.join("target.txt"), b"hello").unwrap();
        unix_fs::symlink("target.txt", source.join("link.txt")).unwrap();

        let output = dir.path().join("out.squashfs");
        let source_utf8 = Utf8Path::from_path(&source).unwrap();
        let output_utf8 = Utf8Path::from_path(&output).unwrap();
        create_squashfs(source_utf8, output_utf8).unwrap();

        // Read back and verify the symlink is stored correctly
        let file = BufReader::new(File::open(&output).unwrap());
        let reader = FilesystemReader::from_reader(file).unwrap();

        let mut found_symlink = false;
        for node in reader.files() {
            if node.fullpath.ends_with("link.txt")
                && let InnerNode::Symlink(symlink) = &node.inner
            {
                assert_eq!(symlink.link.to_string_lossy(), "target.txt");
                found_symlink = true;
            }
        }
        assert!(found_symlink, "symlink entry not found in squashfs");
    }

    #[test]
    fn create_squashfs_with_directory_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(source.join("subdir")).unwrap();

        fs::write(source.join("file.txt"), b"content").unwrap();
        unix_fs::symlink("subdir", source.join("link_to_dir")).unwrap();

        let output = dir.path().join("out.squashfs");
        let source_utf8 = Utf8Path::from_path(&source).unwrap();
        let output_utf8 = Utf8Path::from_path(&output).unwrap();
        create_squashfs(source_utf8, output_utf8).unwrap();

        let file = BufReader::new(File::open(&output).unwrap());
        let reader = FilesystemReader::from_reader(file).unwrap();

        let mut has_dir = false;
        let mut has_file = false;
        let mut has_symlink = false;
        for node in reader.files() {
            if node.fullpath.ends_with("subdir") {
                has_dir = matches!(&node.inner, InnerNode::Dir(_));
            }
            if node.fullpath.ends_with("file.txt") {
                has_file = matches!(&node.inner, InnerNode::File(_));
            }
            if node.fullpath.ends_with("link_to_dir") {
                has_symlink = matches!(&node.inner, InnerNode::Symlink(_));
            }
        }
        assert!(has_dir, "directory not found");
        assert!(has_file, "regular file not found");
        assert!(has_symlink, "symlink not found");
    }

    #[test]
    fn create_squashfs_dangling_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(&source).unwrap();

        // Dangling symlink — target does not exist
        unix_fs::symlink("nonexistent", source.join("dangling")).unwrap();

        let output = dir.path().join("out.squashfs");
        let source_utf8 = Utf8Path::from_path(&source).unwrap();
        let output_utf8 = Utf8Path::from_path(&output).unwrap();
        create_squashfs(source_utf8, output_utf8).unwrap();

        let file = BufReader::new(File::open(&output).unwrap());
        let reader = FilesystemReader::from_reader(file).unwrap();

        let mut found = false;
        for node in reader.files() {
            if node.fullpath.ends_with("dangling")
                && let InnerNode::Symlink(symlink) = &node.inner
            {
                assert_eq!(symlink.link.to_string_lossy(), "nonexistent");
                found = true;
            }
        }
        assert!(found, "dangling symlink not stored in squashfs");
    }
}
