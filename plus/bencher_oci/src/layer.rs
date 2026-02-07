//! OCI layer extraction.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use camino::Utf8Path;
use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

use crate::error::OciError;

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

    // Double-check: the canonical prefix must still be target_dir.
    // We can't canonicalize (target may not exist yet), but the component check above
    // is sufficient since we already rejected all `..` segments.
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
    fn safe_join_allows_dot() {
        let dir = Utf8Path::new("/rootfs");
        let result = safe_join(dir, std::path::Path::new("./usr/bin")).unwrap();
        assert_eq!(result, Utf8Path::new("/rootfs/./usr/bin"));
    }
}
