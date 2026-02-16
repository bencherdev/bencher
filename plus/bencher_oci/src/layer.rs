//! OCI layer extraction.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use camino::Utf8Path;
use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

use crate::error::OciError;

/// Normalize a path by collapsing `.` and `..` components lexically.
///
/// Unlike `canonicalize`, this does not touch the filesystem and works on
/// paths that may not yet exist. A leading `..` that cannot be collapsed
/// is preserved so that `safe_join` will still reject it.
fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Pop the last normal component; if there is none, keep the `..`
                // so that `safe_join` can reject it as a traversal attempt.
                if matches!(components.last(), Some(std::path::Component::Normal(_))) {
                    components.pop();
                } else {
                    components.push(component);
                }
            },
            std::path::Component::CurDir => {},
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

/// Safely join a path component to a target directory, preventing path traversal.
///
/// Strips leading `/` and rejects any path that would escape `target_dir`
/// via `..` components.
fn safe_join(
    target_dir: &Utf8Path,
    entry_path: &std::path::Path,
) -> Result<camino::Utf8PathBuf, OciError> {
    // Convert to string, strip leading /
    let entry_str = entry_path.to_string_lossy();
    let stripped = entry_str.strip_prefix('/').unwrap_or(&entry_str);

    // Reject any path containing .. components
    for component in std::path::Path::new(stripped).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(OciError::PathTraversal(format!(
                "Path contains `..`: {}",
                entry_path.display()
            )));
        }
    }

    let joined = target_dir.join(stripped);

    // Defense-in-depth: verify the joined path starts with the target directory
    if !joined.starts_with(target_dir) {
        return Err(OciError::PathTraversal(format!(
            "Path escapes target directory: {}",
            entry_path.display()
        )));
    }

    Ok(joined)
}

/// Layer compression type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCompression {
    /// Uncompressed tar.
    None,

    /// Gzip compressed.
    Gzip,

    /// Zstd compressed.
    Zstd,
}

/// Extract a layer to a directory.
///
/// # Arguments
///
/// * `layer_path` - Path to the layer blob
/// * `target_dir` - Directory to extract to
/// * `compression` - Compression type
pub fn extract_layer(
    layer_path: &Utf8Path,
    target_dir: &Utf8Path,
    compression: LayerCompression,
) -> Result<(), OciError> {
    let file = File::open(layer_path)?;
    let reader = BufReader::new(file);

    match compression {
        LayerCompression::None => {
            extract_tar(reader, target_dir)?;
        },
        LayerCompression::Gzip => {
            let decoder = GzDecoder::new(reader);
            extract_tar(decoder, target_dir)?;
        },
        LayerCompression::Zstd => {
            let decoder = zstd::Decoder::new(reader)?;
            extract_tar(decoder, target_dir)?;
        },
    }

    Ok(())
}

/// A deferred hard link to create after all regular files are extracted.
struct DeferredHardLink {
    /// Path to create the link at.
    link_path: PathBuf,
    /// Path the link should point to.
    link_target: PathBuf,
}

/// A deferred directory permission to apply after all entries are extracted.
///
/// Directory permissions must be applied after extraction because
/// `create_dir_all` pre-creates intermediate directories with default mode,
/// and `entry.unpack()` may not reliably re-apply permissions on
/// already-existing directories across all entry orderings.
struct DeferredDirPermission {
    /// Path to the directory.
    path: PathBuf,
    /// Permission mode from the tar header.
    mode: u32,
}

/// Extract a tar archive to a directory.
///
/// Hard links are deferred until after all regular files are extracted,
/// since the link target might appear later in the archive.
fn extract_tar<R: Read>(reader: R, target_dir: &Utf8Path) -> Result<(), OciError> {
    let mut archive = Archive::new(reader);

    // Set options for extraction
    archive.set_overwrite(true);
    archive.set_preserve_permissions(true);
    archive.set_unpack_xattrs(true);

    // Collect hard links to create after all regular files are extracted
    let mut deferred_hardlinks: Vec<DeferredHardLink> = Vec::new();

    // Collect directory permissions to apply after all entries are extracted.
    // This is needed because `create_dir_all` pre-creates intermediate directories
    // with default mode (0o755 via umask), and `entry.unpack()` may not reliably
    // re-apply permissions on already-existing directories.
    let mut deferred_dir_perms: Vec<DeferredDirPermission> = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Handle whiteout files (deletions)
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with(".wh.")
        {
            handle_whiteout(target_dir, &path)?;
            continue;
        }

        let target_path = safe_join(target_dir, &path)?;

        // Create parent directories if needed
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Check if this is a hard link - defer it for later
        if entry.header().entry_type() == EntryType::Link {
            if let Ok(Some(link_name)) = entry.link_name() {
                let link_target = safe_join(target_dir, &link_name)?;
                deferred_hardlinks.push(DeferredHardLink {
                    link_path: target_path.into_std_path_buf(),
                    link_target: link_target.into_std_path_buf(),
                });
            }
            continue;
        }

        // Validate symlink targets stay within the target directory
        if entry.header().entry_type() == EntryType::Symlink
            && let Ok(Some(link_name)) = entry.link_name()
        {
            let resolved = if link_name.is_absolute() {
                // Absolute symlink target: resolve relative to target_dir
                link_name.to_path_buf()
            } else {
                // Relative symlink target: resolve relative to the entry's parent
                path.parent()
                    .unwrap_or(std::path::Path::new(""))
                    .join(&link_name)
            };
            // Normalize the path (collapse `..` components) before checking
            // traversal. Symlinks like `usr/bin/../../bin/env` are legitimate
            // and resolve to `bin/env` which stays inside the rootfs.
            let normalized = normalize_path(&resolved);
            safe_join(target_dir, &normalized)?;
        }

        // Record directory permissions for deferred application
        if entry.header().entry_type() == EntryType::Directory
            && let Ok(mode) = entry.header().mode()
        {
            deferred_dir_perms.push(DeferredDirPermission {
                path: target_path.clone().into_std_path_buf(),
                mode,
            });
        }

        // Extract regular files, directories, symlinks, etc.
        entry.unpack(&target_path).map_err(|e| {
            OciError::LayerExtraction(format!("Failed to extract {}: {e}", path.display()))
        })?;
    }

    // Now create all deferred hard links
    for hardlink in deferred_hardlinks {
        std::fs::hard_link(&hardlink.link_target, &hardlink.link_path).map_err(|e| {
            OciError::LayerExtraction(format!(
                "Failed to create hard link from {} to {}: {e}",
                hardlink.link_target.display(),
                hardlink.link_path.display()
            ))
        })?;
    }

    // Apply deferred directory permissions (deepest first so parent permission
    // restrictions don't block child updates)
    deferred_dir_perms.sort_by(|a, b| {
        b.path
            .components()
            .count()
            .cmp(&a.path.components().count())
    });
    for dir_perm in &deferred_dir_perms {
        use std::os::unix::fs::PermissionsExt as _;
        let perms = std::fs::Permissions::from_mode(dir_perm.mode);
        std::fs::set_permissions(&dir_perm.path, perms)?;
    }

    Ok(())
}

/// Handle OCI whiteout files.
///
/// Whiteout files indicate that a file should be deleted in this layer.
/// Format: `.wh.<filename>` - delete that file
/// Special: `.wh..wh..opq` - delete all files in the directory (opaque)
fn handle_whiteout(target_dir: &Utf8Path, whiteout_path: &std::path::Path) -> Result<(), OciError> {
    let name = whiteout_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| OciError::LayerExtraction("Invalid whiteout path".to_owned()))?;

    let parent = whiteout_path.parent();

    if name == ".wh..wh..opq" {
        // Opaque whiteout: clear the directory
        if let Some(parent) = parent {
            let target_path = safe_join(target_dir, parent)?;
            if target_path.exists() {
                // Remove all contents but keep the directory
                for entry in std::fs::read_dir(&target_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        std::fs::remove_dir_all(&path)?;
                    } else {
                        std::fs::remove_file(&path)?;
                    }
                }
            }
        }
    } else if let Some(original_name) = name.strip_prefix(".wh.") {
        // Regular whiteout: delete the specific file
        let whiteout_target = if let Some(parent) = parent {
            parent.join(original_name)
        } else {
            PathBuf::from(original_name)
        };
        let target_path = safe_join(target_dir, &whiteout_target)?;

        if target_path.exists() {
            if target_path.is_dir() {
                std::fs::remove_dir_all(&target_path)?;
            } else {
                std::fs::remove_file(&target_path)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_compression_enum() {
        assert_ne!(LayerCompression::None, LayerCompression::Gzip);
        assert_ne!(LayerCompression::Gzip, LayerCompression::Zstd);
    }

    #[test]
    fn safe_join_normal_path() {
        let dir = Utf8Path::new("/rootfs");
        let result = safe_join(dir, std::path::Path::new("usr/bin/hello")).unwrap();
        assert_eq!(result, Utf8Path::new("/rootfs/usr/bin/hello"));
    }

    #[test]
    fn safe_join_strips_leading_slash() {
        let dir = Utf8Path::new("/rootfs");
        let result = safe_join(dir, std::path::Path::new("/etc/passwd")).unwrap();
        assert_eq!(result, Utf8Path::new("/rootfs/etc/passwd"));
    }

    #[test]
    fn safe_join_rejects_dotdot() {
        let dir = Utf8Path::new("/rootfs");
        assert!(safe_join(dir, std::path::Path::new("../../etc/passwd")).is_err());
    }

    #[test]
    fn safe_join_rejects_nested_dotdot() {
        let dir = Utf8Path::new("/rootfs");
        assert!(safe_join(dir, std::path::Path::new("foo/../../bar")).is_err());
    }

    #[test]
    fn safe_join_rejects_absolute_with_dotdot() {
        let dir = Utf8Path::new("/rootfs");
        assert!(safe_join(dir, std::path::Path::new("/foo/../../../etc/shadow")).is_err());
    }

    #[test]
    fn safe_join_result_starts_with_target() {
        let dir = Utf8Path::new("/rootfs");
        let result = safe_join(dir, std::path::Path::new("usr/bin/hello")).unwrap();
        assert!(result.starts_with(dir));
    }

    #[test]
    fn safe_join_allows_dot() {
        let dir = Utf8Path::new("/rootfs");
        let result = safe_join(dir, std::path::Path::new("./usr/bin")).unwrap();
        assert_eq!(result, Utf8Path::new("/rootfs/./usr/bin"));
    }

    #[test]
    fn test_directory_permissions_preserved() {
        use std::os::unix::fs::PermissionsExt;
        use tar::{Builder, Header};

        let temp_dir = tempfile::tempdir().unwrap();
        let tar_path = temp_dir.path().join("test.tar");
        let extract_dir = temp_dir.path().join("extract");
        std::fs::create_dir_all(&extract_dir).unwrap();

        // Create a tar with a directory at mode 0o750
        {
            let tar_file = File::create(&tar_path).unwrap();
            let mut builder = Builder::new(tar_file);

            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_mode(0o750);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, "mydir/", std::io::empty())
                .unwrap();

            builder.finish().unwrap();
        }

        // Extract
        let reader = File::open(&tar_path).unwrap();
        let target = Utf8Path::from_path(&extract_dir).unwrap();
        extract_tar(reader, target).unwrap();

        // Verify permissions
        let meta = std::fs::metadata(extract_dir.join("mydir")).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o750, "Directory mode should be 0o750, got {mode:#o}");
    }

    #[test]
    fn test_file_permissions_preserved() {
        use std::os::unix::fs::PermissionsExt;
        use tar::{Builder, Header};

        let temp_dir = tempfile::tempdir().unwrap();
        let tar_path = temp_dir.path().join("test.tar");
        let extract_dir = temp_dir.path().join("extract");
        std::fs::create_dir_all(&extract_dir).unwrap();

        // Create a tar with a file at mode 0o755 (executable)
        let content = b"#!/bin/sh\necho hello";
        {
            let tar_file = File::create(&tar_path).unwrap();
            let mut builder = Builder::new(tar_file);

            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_mode(0o755);
            header.set_size(content.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "test.sh", &content[..])
                .unwrap();

            builder.finish().unwrap();
        }

        // Extract
        let reader = File::open(&tar_path).unwrap();
        let target = Utf8Path::from_path(&extract_dir).unwrap();
        extract_tar(reader, target).unwrap();

        // Verify executable bit is set
        let meta = std::fs::metadata(extract_dir.join("test.sh")).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "File mode should be 0o755, got {mode:#o}");
    }

    #[test]
    fn test_nested_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;
        use tar::{Builder, Header};

        let temp_dir = tempfile::tempdir().unwrap();
        let tar_path = temp_dir.path().join("test.tar");
        let extract_dir = temp_dir.path().join("extract");
        std::fs::create_dir_all(&extract_dir).unwrap();

        // Create a tar with nested dirs at different modes
        {
            let tar_file = File::create(&tar_path).unwrap();
            let mut builder = Builder::new(tar_file);

            // Parent directory at 0o755
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_mode(0o755);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, "parent/", std::io::empty())
                .unwrap();

            // Child directory at 0o700
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Directory);
            header.set_mode(0o700);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, "parent/child/", std::io::empty())
                .unwrap();

            builder.finish().unwrap();
        }

        // Extract
        let reader = File::open(&tar_path).unwrap();
        let target = Utf8Path::from_path(&extract_dir).unwrap();
        extract_tar(reader, target).unwrap();

        // Verify both directory permissions
        let parent_mode = std::fs::metadata(extract_dir.join("parent"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let child_mode = std::fs::metadata(extract_dir.join("parent/child"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(
            parent_mode, 0o755,
            "Parent mode should be 0o755, got {parent_mode:#o}"
        );
        assert_eq!(
            child_mode, 0o700,
            "Child mode should be 0o700, got {child_mode:#o}"
        );
    }
}
