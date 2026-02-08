//! Typed OCI and Docker manifest schemas
//!
//! These types represent the wire format for OCI Distribution Spec manifests.
//! They are used for validation, content-type extraction, and subject extraction
//! when manifests are pushed to or pulled from the registry.
//!
//! References:
//! - OCI Image Manifest: <https://github.com/opencontainers/image-spec/blob/main/manifest.md>
//! - OCI Image Index: <https://github.com/opencontainers/image-spec/blob/main/image-index.md>
//! - OCI Content Descriptor: <https://github.com/opencontainers/image-spec/blob/main/descriptor.md>
//! - Docker Manifest V2: <https://distribution.github.io/distribution/spec/manifest-v2-2/>

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Default OCI image manifest media type
pub const OCI_IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
/// OCI image index media type
pub const OCI_IMAGE_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
/// Docker manifest V2 media type
pub const DOCKER_MANIFEST_V2_MEDIA_TYPE: &str =
    "application/vnd.docker.distribution.manifest.v2+json";
/// Docker manifest list media type
pub const DOCKER_MANIFEST_LIST_MEDIA_TYPE: &str =
    "application/vnd.docker.distribution.manifest.list.v2+json";

/// OCI Content Descriptor
///
/// A descriptor references content by digest and declares its media type and size.
/// Used for config, layers, and manifest references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciDescriptor {
    /// Media type of the referenced content (REQUIRED)
    pub media_type: String,
    /// Digest of the referenced content (REQUIRED)
    pub digest: String,
    /// Size in bytes of the referenced content (REQUIRED)
    pub size: i64,
    /// URLs from which the content may be fetched
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    /// Arbitrary metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    /// Base64-encoded content (for small embedded data)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Type of artifact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
}

/// Platform describes the platform which the image in the manifest runs on
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    /// CPU architecture (REQUIRED)
    pub architecture: String,
    /// Operating system (REQUIRED)
    pub os: String,
    /// OS version
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "os.version"
    )]
    pub os_version: Option<String>,
    /// OS features
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "os.features"
    )]
    pub os_features: Option<Vec<String>>,
    /// CPU variant
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Manifest descriptor with optional platform (for image index/manifest list entries)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciManifestDescriptor {
    /// Media type of the referenced manifest (REQUIRED)
    pub media_type: String,
    /// Digest of the referenced manifest (REQUIRED)
    pub digest: String,
    /// Size in bytes of the referenced manifest (REQUIRED)
    pub size: i64,
    /// URLs from which the content may be fetched
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    /// Arbitrary metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    /// Platform the manifest is built for
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
    /// Artifact type of the referenced manifest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
}

/// OCI Image Manifest
///
/// Describes the components of a single container image.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageManifest {
    /// Schema version (REQUIRED, must be 2)
    pub schema_version: u32,
    /// Media type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Configuration object (REQUIRED)
    pub config: OciDescriptor,
    /// Ordered list of layers (REQUIRED)
    pub layers: Vec<OciDescriptor>,
    /// Reference to another manifest (for referrers API)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<OciDescriptor>,
    /// Arbitrary metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    /// Type of artifact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
}

/// OCI Image Index (multi-platform manifest)
///
/// Points to platform-specific manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageIndex {
    /// Schema version (REQUIRED, must be 2)
    pub schema_version: u32,
    /// Media type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// List of manifest descriptors (REQUIRED, MAY be empty)
    pub manifests: Vec<OciManifestDescriptor>,
    /// Reference to another manifest (for referrers API)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<OciDescriptor>,
    /// Arbitrary metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
    /// Type of artifact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
}

/// Docker Manifest V2, Schema 2
///
/// Docker's container image manifest format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerManifestV2 {
    /// Schema version (REQUIRED, must be 2)
    pub schema_version: u32,
    /// Media type (REQUIRED)
    pub media_type: String,
    /// Configuration object (REQUIRED)
    pub config: OciDescriptor,
    /// Ordered list of layers (REQUIRED)
    pub layers: Vec<OciDescriptor>,
}

/// Docker Manifest List (multi-platform)
///
/// Docker's multi-platform manifest format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerManifestList {
    /// Schema version (REQUIRED, must be 2)
    pub schema_version: u32,
    /// Media type (REQUIRED)
    pub media_type: String,
    /// Platform-specific manifest references (REQUIRED)
    pub manifests: Vec<OciManifestDescriptor>,
}

/// A parsed OCI/Docker manifest
///
/// Wraps all supported manifest types. Deserialization is based on the `mediaType` field.
#[derive(Debug, Clone)]
pub enum Manifest {
    OciImageManifest(OciImageManifest),
    OciImageIndex(OciImageIndex),
    DockerManifestV2(DockerManifestV2),
    DockerManifestList(DockerManifestList),
}

impl Manifest {
    /// Parse manifest bytes into a typed manifest
    ///
    /// First extracts the `mediaType` field to determine which type to deserialize as.
    /// Falls back to `OciImageManifest` if `mediaType` is absent (per OCI spec recommendation).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        // First pass: extract mediaType to determine the manifest type
        #[derive(Deserialize)]
        struct MediaTypeProbe {
            #[serde(default, rename = "mediaType")]
            media_type: Option<String>,
        }

        let probe: MediaTypeProbe = serde_json::from_slice(bytes)?;

        let manifest = match probe.media_type.as_deref() {
            Some(DOCKER_MANIFEST_V2_MEDIA_TYPE) => {
                serde_json::from_slice(bytes).map(Self::DockerManifestV2)
            },
            Some(DOCKER_MANIFEST_LIST_MEDIA_TYPE) => {
                serde_json::from_slice(bytes).map(Self::DockerManifestList)
            },
            Some(OCI_IMAGE_INDEX_MEDIA_TYPE) => {
                serde_json::from_slice(bytes).map(Self::OciImageIndex)
            },
            // Default to OCI Image Manifest (covers both explicit and absent mediaType)
            Some(OCI_IMAGE_MANIFEST_MEDIA_TYPE) | None => {
                serde_json::from_slice(bytes).map(Self::OciImageManifest)
            },
            Some(unknown) => Err(serde::de::Error::custom(format!(
                "unsupported manifest mediaType: {unknown}"
            ))),
        }?;

        manifest.validate().map_err(serde::de::Error::custom)?;

        Ok(manifest)
    }

    /// Validates schemaVersion and that all descriptor sizes are non-negative per OCI spec
    fn validate(&self) -> Result<(), String> {
        fn check_descriptor(desc: &OciDescriptor, context: &str) -> Result<(), String> {
            if desc.size < 0 {
                return Err(format!(
                    "{context} has negative size: {size}",
                    size = desc.size
                ));
            }
            Ok(())
        }

        fn check_manifest_descriptor(
            desc: &OciManifestDescriptor,
            context: &str,
        ) -> Result<(), String> {
            if desc.size < 0 {
                return Err(format!(
                    "{context} has negative size: {size}",
                    size = desc.size
                ));
            }
            Ok(())
        }

        let sv = match self {
            Self::OciImageManifest(m) => m.schema_version,
            Self::OciImageIndex(m) => m.schema_version,
            Self::DockerManifestV2(m) => m.schema_version,
            Self::DockerManifestList(m) => m.schema_version,
        };
        if sv != 2 {
            return Err(format!("schemaVersion must be 2, got {sv}"));
        }

        match self {
            Self::OciImageManifest(m) => {
                check_descriptor(&m.config, "config")?;
                for (i, layer) in m.layers.iter().enumerate() {
                    check_descriptor(layer, &format!("layers[{i}]"))?;
                }
                if let Some(subject) = &m.subject {
                    check_descriptor(subject, "subject")?;
                }
            },
            Self::OciImageIndex(m) => {
                for (i, manifest) in m.manifests.iter().enumerate() {
                    check_manifest_descriptor(manifest, &format!("manifests[{i}]"))?;
                }
                if let Some(subject) = &m.subject {
                    check_descriptor(subject, "subject")?;
                }
            },
            Self::DockerManifestV2(m) => {
                check_descriptor(&m.config, "config")?;
                for (i, layer) in m.layers.iter().enumerate() {
                    check_descriptor(layer, &format!("layers[{i}]"))?;
                }
            },
            Self::DockerManifestList(m) => {
                for (i, manifest) in m.manifests.iter().enumerate() {
                    check_manifest_descriptor(manifest, &format!("manifests[{i}]"))?;
                }
            },
        }
        Ok(())
    }

    /// Returns the media type string for this manifest
    pub fn media_type(&self) -> &str {
        match self {
            Self::OciImageManifest(m) => m
                .media_type
                .as_deref()
                .unwrap_or(OCI_IMAGE_MANIFEST_MEDIA_TYPE),
            Self::OciImageIndex(m) => m
                .media_type
                .as_deref()
                .unwrap_or(OCI_IMAGE_INDEX_MEDIA_TYPE),
            Self::DockerManifestV2(m) => &m.media_type,
            Self::DockerManifestList(m) => &m.media_type,
        }
    }

    /// Returns the subject descriptor if present (for OCI referrers API)
    pub fn subject(&self) -> Option<&OciDescriptor> {
        match self {
            Self::OciImageManifest(m) => m.subject.as_ref(),
            Self::OciImageIndex(m) => m.subject.as_ref(),
            // Docker manifests don't have a subject field
            Self::DockerManifestV2(_) | Self::DockerManifestList(_) => None,
        }
    }

    /// Returns the artifact type if present
    pub fn artifact_type(&self) -> Option<&str> {
        match self {
            Self::OciImageManifest(m) => m
                .artifact_type
                .as_deref()
                // Fall back to config.mediaType per OCI spec
                .or(Some(m.config.media_type.as_str())),
            Self::OciImageIndex(m) => m.artifact_type.as_deref(),
            Self::DockerManifestV2(_) | Self::DockerManifestList(_) => None,
        }
    }

    /// Returns annotations if present
    pub fn annotations(&self) -> Option<&HashMap<String, String>> {
        match self {
            Self::OciImageManifest(m) => m.annotations.as_ref(),
            Self::OciImageIndex(m) => m.annotations.as_ref(),
            Self::DockerManifestV2(_) | Self::DockerManifestList(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oci_image_manifest() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 1234
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                    "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    "size": 5678
                }
            ]
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.media_type(), OCI_IMAGE_MANIFEST_MEDIA_TYPE);
        assert!(manifest.subject().is_none());
    }

    #[test]
    fn parse_oci_image_manifest_with_subject() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 1234
            },
            "layers": [],
            "subject": {
                "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
                "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "size": 999
            }
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        let subject = manifest.subject().unwrap();
        assert_eq!(
            subject.digest,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn parse_oci_image_index() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_INDEX_MEDIA_TYPE,
            "manifests": [
                {
                    "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
                    "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    "size": 1234,
                    "platform": {
                        "architecture": "amd64",
                        "os": "linux"
                    }
                }
            ]
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.media_type(), OCI_IMAGE_INDEX_MEDIA_TYPE);
    }

    #[test]
    fn parse_docker_manifest_v2() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": DOCKER_MANIFEST_V2_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.docker.container.image.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 1234
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.media_type(), DOCKER_MANIFEST_V2_MEDIA_TYPE);
    }

    #[test]
    fn parse_docker_manifest_list() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": DOCKER_MANIFEST_LIST_MEDIA_TYPE,
            "manifests": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.media_type(), DOCKER_MANIFEST_LIST_MEDIA_TYPE);
    }

    #[test]
    fn parse_manifest_without_media_type_defaults_to_oci() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 1234
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.media_type(), OCI_IMAGE_MANIFEST_MEDIA_TYPE);
    }

    #[test]
    fn parse_invalid_json_fails() {
        assert!(Manifest::from_bytes(b"not json").is_err());
    }

    #[test]
    fn parse_unsupported_media_type_fails() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.unknown.type"
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_missing_required_fields_fails() {
        // OCI image manifest requires config and layers
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_manifest_negative_config_size() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": -1
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_manifest_negative_layer_size() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 1234
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
                    "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    "size": -500
                }
            ]
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_manifest_zero_size_ok() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 0
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_ok());
    }

    #[test]
    fn parse_manifest_schema_version_0() {
        let json = serde_json::json!({
            "schemaVersion": 0,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 0
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_manifest_schema_version_1() {
        let json = serde_json::json!({
            "schemaVersion": 1,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 0
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn parse_manifest_schema_version_3() {
        let json = serde_json::json!({
            "schemaVersion": 3,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 0
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(Manifest::from_bytes(&bytes).is_err());
    }

    #[test]
    fn artifact_type_fallback_to_config_media_type() {
        let json = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            "config": {
                "mediaType": "application/vnd.example.config.v1+json",
                "digest": "sha256:44136fa355b311bfa0680e24bf37c9e4e6e2b637bfb8e6e1e9bfb7e7e9bfb7e7",
                "size": 0
            },
            "layers": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let manifest = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(
            manifest.artifact_type(),
            Some("application/vnd.example.config.v1+json")
        );
    }
}
