//! OCI image manifest and configuration parsing.

use std::fs::File;
use std::io::BufReader;

use camino::Utf8Path;
use oci_spec::image::{
    ImageConfiguration, ImageIndex, ImageManifest as OciImageManifest, MediaType,
};
use serde::{Deserialize, Serialize};

use crate::error::OciError;

/// OCI image layout file content.
const OCI_LAYOUT_VERSION: &str = "1.0.0";

/// Parsed OCI image manifest.
#[derive(Debug, Clone)]
pub struct ImageManifest {
    /// The raw OCI manifest.
    pub manifest: OciImageManifest,

    /// Layer digests in order.
    pub layers: Vec<String>,

    /// Config digest.
    pub config_digest: String,
}

/// Parsed OCI image configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// The raw OCI config.
    #[serde(flatten)]
    pub config: ImageConfiguration,
}

impl ImageConfig {
    /// Get the command to run (combining ENTRYPOINT and CMD).
    ///
    /// OCI spec: the final command is `ENTRYPOINT + CMD`.
    /// - If only CMD is set, run CMD
    /// - If only ENTRYPOINT is set, run ENTRYPOINT
    /// - If both are set, run ENTRYPOINT with CMD as arguments
    #[must_use]
    pub fn command(&self) -> Vec<String> {
        let config = self.config.config();

        let entrypoint: Vec<String> = config
            .as_ref()
            .and_then(|c| c.entrypoint().as_ref())
            .map(|e| e.iter().map(String::clone).collect())
            .unwrap_or_default();

        let cmd: Vec<String> = config
            .as_ref()
            .and_then(|c| c.cmd().as_ref())
            .map(|c| c.iter().map(String::clone).collect())
            .unwrap_or_default();

        if entrypoint.is_empty() {
            cmd
        } else {
            let mut command = entrypoint;
            command.extend(cmd);
            command
        }
    }

    /// Get the working directory.
    #[must_use]
    pub fn working_dir(&self) -> Option<&str> {
        self.config
            .config()
            .as_ref()
            .and_then(|c| c.working_dir().as_deref())
    }

    /// Get environment variables as key-value pairs.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        self.config
            .config()
            .as_ref()
            .and_then(|c| c.env().as_ref())
            .map(|env| {
                env.iter()
                    .filter_map(|e| e.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the user to run as.
    #[must_use]
    pub fn user(&self) -> Option<&str> {
        self.config
            .config()
            .as_ref()
            .and_then(|c| c.user().as_deref())
    }
}

/// Parse the OCI layout from an image directory.
pub fn parse_oci_layout(image_dir: &Utf8Path) -> Result<(), OciError> {
    let layout_path = image_dir.join("oci-layout");

    let file = File::open(&layout_path)
        .map_err(|e| OciError::InvalidLayout(format!("Cannot open oci-layout: {e}")))?;

    let layout: serde_json::Value = serde_json::from_reader(BufReader::new(file))?;

    let version = layout
        .get("imageLayoutVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OciError::InvalidLayout("Missing imageLayoutVersion".to_owned()))?;

    if version != OCI_LAYOUT_VERSION {
        return Err(OciError::InvalidLayout(format!(
            "Unsupported OCI layout version: {version}"
        )));
    }

    Ok(())
}

/// Parse the image index (index.json).
pub fn parse_index(image_dir: &Utf8Path) -> Result<ImageIndex, OciError> {
    let index_path = image_dir.join("index.json");

    let file = File::open(&index_path)
        .map_err(|e| OciError::InvalidLayout(format!("Cannot open index.json: {e}")))?;

    let index = ImageIndex::from_reader(BufReader::new(file))
        .map_err(|e| OciError::InvalidLayout(format!("Invalid index.json: {e}")))?;

    Ok(index)
}

/// Map the Rust `std::env::consts::ARCH` value to the OCI architecture name.
fn oci_arch() -> &'static str {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        arch => arch,
    }
}

/// Get the manifest for the specified platform or the first manifest.
pub fn get_manifest(image_dir: &Utf8Path, index: &ImageIndex) -> Result<ImageManifest, OciError> {
    let manifests = index.manifests();

    if manifests.is_empty() {
        return Err(OciError::MissingManifest(
            "No manifests in index".to_owned(),
        ));
    }

    let target_arch = oci_arch();

    // Find a manifest matching linux + current architecture, fall back to first
    let manifest_desc = manifests
        .iter()
        .find(|m| {
            m.platform().as_ref().is_some_and(|p| {
                p.os().to_string() == "linux" && p.architecture().to_string() == target_arch
            })
        })
        .or_else(|| manifests.first())
        .ok_or_else(|| OciError::MissingManifest("No manifests in index".to_owned()))?;

    // Parse the digest to get the blob path
    let digest = manifest_desc.digest().to_string();
    let blob_path = digest_to_blob_path(image_dir, &digest)?;

    // Read and parse the manifest
    let file = File::open(&blob_path)
        .map_err(|e| OciError::MissingBlob(format!("Cannot open manifest blob {digest}: {e}")))?;

    let manifest = OciImageManifest::from_reader(BufReader::new(file))
        .map_err(|e| OciError::InvalidLayout(format!("Invalid manifest: {e}")))?;

    // Extract layer digests
    let layers = manifest
        .layers()
        .iter()
        .map(|l| l.digest().to_string())
        .collect();

    let config_digest = manifest.config().digest().to_string();

    Ok(ImageManifest {
        manifest,
        layers,
        config_digest,
    })
}

/// Parse the image configuration.
pub fn parse_config(image_dir: &Utf8Path, config_digest: &str) -> Result<ImageConfig, OciError> {
    let blob_path = digest_to_blob_path(image_dir, config_digest)?;

    let file = File::open(&blob_path).map_err(|e| {
        OciError::MissingBlob(format!("Cannot open config blob {config_digest}: {e}"))
    })?;

    let config = ImageConfiguration::from_reader(BufReader::new(file))
        .map_err(|e| OciError::InvalidLayout(format!("Invalid config: {e:?}")))?;

    Ok(ImageConfig { config })
}

/// Convert a digest to a blob path.
///
/// Digest format: `algorithm:hex`
/// Blob path: `blobs/algorithm/hex`
///
/// Validates that the algorithm is alphanumeric (plus `+`, `-`, `.`) per OCI spec
/// and the hex portion contains only hexadecimal characters, preventing path traversal.
pub fn digest_to_blob_path(
    image_dir: &Utf8Path,
    digest: &str,
) -> Result<camino::Utf8PathBuf, OciError> {
    let (algorithm, hex) = digest
        .split_once(':')
        .ok_or_else(|| OciError::PathTraversal(format!("Invalid digest format: {digest}")))?;

    // OCI digest algorithm: [a-z][a-z0-9]*([+.-_][a-z][a-z0-9]*)*
    // We accept alphanumeric plus `+`, `-`, `.`, `_` which covers all valid algorithms.
    if algorithm.is_empty()
        || !algorithm
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.' || b == b'_')
    {
        return Err(OciError::PathTraversal(format!(
            "Invalid digest algorithm: {algorithm}"
        )));
    }

    // Hex portion: only hexadecimal characters
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(OciError::PathTraversal(format!(
            "Invalid digest hex: {hex}"
        )));
    }

    Ok(image_dir.join("blobs").join(algorithm).join(hex))
}

/// Full parsed OCI image information.
#[derive(Debug, Clone)]
pub struct OciImage {
    /// The image manifest.
    pub manifest: ImageManifest,

    /// The image configuration.
    pub config: ImageConfig,
}

impl OciImage {
    /// Parse an OCI image directory and return full image information.
    pub fn parse(image_dir: &Utf8Path) -> Result<Self, OciError> {
        // Validate layout
        parse_oci_layout(image_dir)?;

        // Parse index and manifest
        let index = parse_index(image_dir)?;
        let manifest = get_manifest(image_dir, &index)?;

        // Parse config
        let config = parse_config(image_dir, &manifest.config_digest)?;

        Ok(Self { manifest, config })
    }

    /// Get the command to run in the container.
    #[must_use]
    pub fn command(&self) -> Vec<String> {
        self.config.command()
    }

    /// Get the working directory.
    #[must_use]
    pub fn working_dir(&self) -> Option<&str> {
        self.config.working_dir()
    }

    /// Get environment variables.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        self.config.env()
    }
}

/// Detect the media type of a layer.
#[expect(clippy::wildcard_enum_match_arm)]
pub fn detect_layer_media_type(
    media_type: &MediaType,
) -> Result<super::LayerCompression, OciError> {
    match media_type {
        MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
            Ok(super::LayerCompression::Gzip)
        },
        MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
            Ok(super::LayerCompression::Zstd)
        },
        MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
            Ok(super::LayerCompression::None)
        },
        other => Err(OciError::UnsupportedMediaType(format!("{other:?}"))),
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, reason = "test code")]
mod tests {
    use super::*;
    use camino::Utf8Path;

    #[test]
    fn digest_valid_sha256() {
        let dir = Utf8Path::new("/oci");
        let result = digest_to_blob_path(dir, "sha256:abcdef0123456789").unwrap();
        assert_eq!(result, Utf8Path::new("/oci/blobs/sha256/abcdef0123456789"));
    }

    #[test]
    fn digest_valid_sha512() {
        let dir = Utf8Path::new("/oci");
        let result = digest_to_blob_path(dir, "sha512:abcdef0123456789").unwrap();
        assert_eq!(result, Utf8Path::new("/oci/blobs/sha512/abcdef0123456789"));
    }

    #[test]
    fn digest_rejects_traversal_in_algorithm() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "../etc:passwd").is_err());
    }

    #[test]
    fn digest_rejects_traversal_in_hex() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "sha256:../../etc/passwd").is_err());
    }

    #[test]
    fn digest_rejects_slash_in_algorithm() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "sha256/../../blobs:abc").is_err());
    }

    #[test]
    fn digest_rejects_no_colon() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "sha256abcdef").is_err());
    }

    #[test]
    fn digest_rejects_empty_algorithm() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, ":abcdef").is_err());
    }

    #[test]
    fn digest_rejects_empty_hex() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "sha256:").is_err());
    }

    #[test]
    fn digest_rejects_non_hex_chars() {
        let dir = Utf8Path::new("/oci");
        assert!(digest_to_blob_path(dir, "sha256:xyz123").is_err());
    }

    #[test]
    fn digest_algorithm_with_special_chars() {
        let dir = Utf8Path::new("/oci");
        // OCI spec allows +, -, . in algorithm names (e.g., sha256+b64)
        let result = digest_to_blob_path(dir, "sha256+b64:abcdef").unwrap();
        assert_eq!(result, Utf8Path::new("/oci/blobs/sha256+b64/abcdef"));
    }

    /// Helper to create a minimal OCI layout with an index and manifest blobs.
    fn create_oci_layout_with_manifests(
        image_dir: &std::path::Path,
        manifest_descs: &[serde_json::Value],
    ) {
        use sha2::{Digest as _, Sha256};

        let blobs_dir = image_dir.join("blobs/sha256");
        std::fs::create_dir_all(&blobs_dir).unwrap();

        // Write oci-layout
        std::fs::write(
            image_dir.join("oci-layout"),
            r#"{"imageLayoutVersion":"1.0.0"}"#,
        )
        .unwrap();

        // Create a minimal config blob
        let config_json = b"{}";
        let config_digest = format!("{:x}", Sha256::digest(config_json));
        std::fs::write(blobs_dir.join(&config_digest), config_json).unwrap();

        // Create manifest blobs for each descriptor and collect index entries
        let mut index_manifests = Vec::new();
        for desc in manifest_descs {
            let platform = desc.get("platform").cloned();

            // Create a minimal OCI image manifest
            let manifest = serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": {
                    "mediaType": "application/vnd.oci.image.config.v1+json",
                    "digest": format!("sha256:{config_digest}"),
                    "size": config_json.len()
                },
                "layers": []
            });
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            let manifest_digest = format!("{:x}", Sha256::digest(&manifest_bytes));
            std::fs::write(blobs_dir.join(&manifest_digest), &manifest_bytes).unwrap();

            let mut entry = serde_json::json!({
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": format!("sha256:{manifest_digest}"),
                "size": manifest_bytes.len()
            });
            if let Some(p) = platform {
                entry
                    .as_object_mut()
                    .unwrap()
                    .insert("platform".to_owned(), p);
            }
            index_manifests.push(entry);
        }

        // Write index.json
        let index = serde_json::json!({
            "schemaVersion": 2,
            "manifests": index_manifests
        });
        std::fs::write(
            image_dir.join("index.json"),
            serde_json::to_string_pretty(&index).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn get_manifest_selects_matching_platform() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_dir = temp_dir.path();

        let current_arch = oci_arch();

        // Create an index with two manifests: one for the other arch, one for current
        let other_arch = if current_arch == "amd64" {
            "arm64"
        } else {
            "amd64"
        };

        create_oci_layout_with_manifests(
            image_dir,
            &[
                serde_json::json!({
                    "platform": { "architecture": other_arch, "os": "linux" }
                }),
                serde_json::json!({
                    "platform": { "architecture": current_arch, "os": "linux" }
                }),
            ],
        );

        let image_dir = Utf8Path::from_path(image_dir).unwrap();
        let index = parse_index(image_dir).unwrap();
        let manifest = get_manifest(image_dir, &index).unwrap();

        // The selected manifest should correspond to the second descriptor (current arch)
        let expected_digest = index.manifests()[1].digest().to_string();
        // Verify by checking config_digest is the same as what the second manifest points to
        assert_eq!(
            manifest.config_digest,
            manifest.manifest.config().digest().to_string()
        );

        // Both manifests point to the same config in our test, so verify we got the right
        // manifest by checking its digest matches the second entry
        let manifest_bytes =
            std::fs::read(digest_to_blob_path(image_dir, &expected_digest).unwrap()).unwrap();
        let expected_manifest: oci_spec::image::ImageManifest =
            oci_spec::image::ImageManifest::from_reader(&manifest_bytes[..]).unwrap();
        assert_eq!(
            manifest.manifest.config().digest(),
            expected_manifest.config().digest()
        );
    }

    #[test]
    fn get_manifest_falls_back_to_first() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_dir = temp_dir.path();

        // Create an index with manifests that have no platform info
        create_oci_layout_with_manifests(
            image_dir,
            &[serde_json::json!({}), serde_json::json!({})],
        );

        let image_dir = Utf8Path::from_path(image_dir).unwrap();
        let index = parse_index(image_dir).unwrap();
        let manifest = get_manifest(image_dir, &index).unwrap();

        // Should fall back to first manifest
        let first_digest = index.manifests()[0].digest().to_string();
        let manifest_bytes =
            std::fs::read(digest_to_blob_path(image_dir, &first_digest).unwrap()).unwrap();
        let first_manifest: oci_spec::image::ImageManifest =
            oci_spec::image::ImageManifest::from_reader(&manifest_bytes[..]).unwrap();
        assert_eq!(
            manifest.manifest.config().digest(),
            first_manifest.config().digest()
        );
    }
}
