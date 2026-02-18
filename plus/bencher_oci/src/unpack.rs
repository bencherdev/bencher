//! Full OCI image unpacking.

use camino::Utf8Path;

use crate::error::OciError;
use crate::image::{
    detect_layer_media_type, digest_to_blob_path, get_manifest, parse_index, parse_oci_layout,
};
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
        let blob_path = digest_to_blob_path(image_dir, &digest)?;

        let compression = detect_layer_media_type(layer.media_type())?;

        tracing_log(format!(
            "Extracting layer {}/{}: {digest}",
            i + 1,
            manifest.layers.len()
        ));

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
    use std::io::Read as _;

    let parsed: bencher_valid::ImageDigest = expected_digest
        .parse()
        .map_err(|_err| OciError::InvalidReference(expected_digest.to_owned()))?;

    let mut file = std::fs::File::open(blob_path)?;
    let mut hasher = crate::digest::DigestHasher::from_algorithm(parsed.algorithm())?;
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(buffer.get(..bytes_read).unwrap_or_default());
    }

    let actual_digest = hasher.finalize();

    if actual_digest != expected_digest {
        return Err(OciError::DigestMismatch {
            expected: expected_digest.to_owned(),
            actual: actual_digest,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::verify_digest;
    use crate::error::OciError;

    #[test]
    fn verify_digest_sha256_valid() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("blob");
        std::fs::write(&file_path, b"").unwrap();
        let path = Utf8Path::from_path(&file_path).unwrap();

        // SHA-256 of empty bytes
        verify_digest(
            path,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap();
    }

    #[test]
    fn verify_digest_sha256_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("blob");
        std::fs::write(&file_path, b"data").unwrap();
        let path = Utf8Path::from_path(&file_path).unwrap();

        let result = verify_digest(
            path,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, OciError::DigestMismatch { .. }),
            "expected DigestMismatch, got {err:?}"
        );
    }

    #[test]
    fn verify_digest_sha512_valid() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("blob");
        std::fs::write(&file_path, b"").unwrap();
        let path = Utf8Path::from_path(&file_path).unwrap();

        // SHA-512 of empty bytes
        verify_digest(
            path,
            "sha512:cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
        )
        .unwrap();
    }

    #[test]
    fn verify_digest_unsupported_algorithm() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("blob");
        std::fs::write(&file_path, b"").unwrap();
        let path = Utf8Path::from_path(&file_path).unwrap();

        // md5 is not a valid ImageDigest, so it fails at parse
        let result = verify_digest(path, "md5:d41d8cd98f00b204e9800998ecf8427e");
        assert!(result.is_err());
    }
}
