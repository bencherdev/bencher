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

/// Parse the OCI layout from an image directory.
pub fn parse_oci_layout(image_dir: &Utf8Path) -> Result<(), OciError> {
    let layout_path = image_dir.join("oci-layout");

    let file = File::open(&layout_path).map_err(|e| {
        OciError::InvalidLayout(format!("Cannot open oci-layout: {e}"))
    })?;

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

    let file = File::open(&index_path).map_err(|e| {
        OciError::InvalidLayout(format!("Cannot open index.json: {e}"))
    })?;

    let index = ImageIndex::from_reader(BufReader::new(file))
        .map_err(|e| OciError::InvalidLayout(format!("Invalid index.json: {e}")))?;

    Ok(index)
}

/// Get the manifest for the specified platform or the first manifest.
pub fn get_manifest(image_dir: &Utf8Path, index: &ImageIndex) -> Result<ImageManifest, OciError> {
    // Get the first manifest descriptor
    let manifest_desc = index
        .manifests()
        .first()
        .ok_or_else(|| OciError::MissingManifest("No manifests in index".to_owned()))?;

    // Parse the digest to get the blob path
    let digest = manifest_desc.digest().to_string();
    let blob_path = digest_to_blob_path(image_dir, &digest);

    // Read and parse the manifest
    let file = File::open(&blob_path).map_err(|e| {
        OciError::MissingBlob(format!("Cannot open manifest blob {digest}: {e}"))
    })?;

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
    let blob_path = digest_to_blob_path(image_dir, config_digest);

    let file = File::open(&blob_path).map_err(|e| {
        OciError::MissingBlob(format!("Cannot open config blob {config_digest}: {e}"))
    })?;

    let config = ImageConfiguration::from_reader(BufReader::new(file))
        .map_err(|e| OciError::InvalidLayout(format!("Invalid config: {e}")))?;

    Ok(ImageConfig { config })
}

/// Convert a digest to a blob path.
///
/// Digest format: `algorithm:hex`
/// Blob path: `blobs/algorithm/hex`
pub fn digest_to_blob_path(image_dir: &Utf8Path, digest: &str) -> camino::Utf8PathBuf {
    let parts: Vec<&str> = digest.splitn(2, ':').collect();
    if parts.len() == 2 {
        image_dir.join("blobs").join(parts[0]).join(parts[1])
    } else {
        // Fallback for malformed digest
        image_dir.join("blobs").join("sha256").join(digest)
    }
}

/// Detect the media type of a layer.
pub fn detect_layer_media_type(media_type: &MediaType) -> Result<super::LayerCompression, OciError> {
    match media_type {
        MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
            Ok(super::LayerCompression::Gzip)
        }
        MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
            Ok(super::LayerCompression::Zstd)
        }
        MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
            Ok(super::LayerCompression::None)
        }
        other => Err(OciError::UnsupportedMediaType(format!("{other:?}"))),
    }
}
