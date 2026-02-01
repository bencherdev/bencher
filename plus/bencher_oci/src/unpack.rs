//! Full OCI image unpacking.

use camino::Utf8Path;

use crate::error::OciError;
use crate::image::{detect_layer_media_type, digest_to_blob_path, get_manifest, parse_index, parse_oci_layout};
use crate::layer::extract_layer;

/// Unpack an OCI image to a directory.
///
/// This function:
/// 1. Validates the OCI layout
/// 2. Parses the image index and manifest
/// 3. Extracts all layers in order
///
/// # Arguments
///
/// * `image_dir` - Path to the OCI image directory (containing oci-layout, index.json, blobs/)
/// * `target_dir` - Directory to unpack the image to
///
/// # Example
///
/// ```ignore
/// use camino::Utf8Path;
/// use bencher_oci::unpack;
///
/// unpack(
///     Utf8Path::new("/path/to/oci-image"),
///     Utf8Path::new("/path/to/rootfs"),
/// )?;
/// ```
pub fn unpack(image_dir: &Utf8Path, target_dir: &Utf8Path) -> Result<(), OciError> {
    // Validate OCI layout
    parse_oci_layout(image_dir)?;

    // Parse image index
    let index = parse_index(image_dir)?;

    // Get the manifest
    let manifest = get_manifest(image_dir, &index)?;

    // Create target directory
    std::fs::create_dir_all(target_dir)?;

    // Extract layers in order
    for (i, layer) in manifest.manifest.layers().iter().enumerate() {
        let digest = layer.digest().to_string();
        let blob_path = digest_to_blob_path(image_dir, &digest);

        let compression = detect_layer_media_type(layer.media_type())?;

        tracing_log(format!("Extracting layer {}/{}: {digest}", i + 1, manifest.layers.len()));

        extract_layer(
            Utf8Path::from_path(blob_path.as_std_path())
                .ok_or_else(|| OciError::InvalidLayout("Invalid blob path".to_owned()))?,
            target_dir,
            compression,
        )?;
    }

    Ok(())
}

/// Simple logging helper (prints to stderr in debug builds).
#[expect(clippy::print_stderr, clippy::needless_pass_by_value)]
fn tracing_log(msg: String) {
    #[cfg(debug_assertions)]
    eprintln!("[bencher_oci] {msg}");

    // Suppress unused variable warning in release builds
    #[cfg(not(debug_assertions))]
    drop(msg);
}

/// Verify a blob's digest.
pub fn verify_digest(blob_path: &Utf8Path, expected_digest: &str) -> Result<(), OciError> {
    use sha2::{Digest as _, Sha256};
    use std::io::Read as _;

    let mut file = std::fs::File::open(blob_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(buffer.get(..bytes_read).unwrap_or_default());
    }

    let hash = hasher.finalize();
    let actual_digest = format!("sha256:{hash:x}");

    if actual_digest != expected_digest {
        return Err(OciError::DigestMismatch {
            expected: expected_digest.to_owned(),
            actual: actual_digest,
        });
    }

    Ok(())
}
