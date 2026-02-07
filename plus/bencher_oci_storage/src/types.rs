//! OCI Registry Types

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A content-addressable digest (e.g., "sha256:abc123...")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Digest(String);

#[derive(Debug, Error)]
pub enum DigestError {
    #[error("Invalid digest format: {0}")]
    InvalidFormat(String),
    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
}

impl Digest {
    /// Expected hex hash length for SHA-256 (64 hex characters = 32 bytes)
    const SHA256_HEX_LEN: usize = 64;
    /// Expected hex hash length for SHA-512 (128 hex characters = 64 bytes)
    const SHA512_HEX_LEN: usize = 128;

    /// Creates a new SHA-256 digest from the given hex-encoded hash
    pub fn sha256(hex_hash: &str) -> Result<Self, DigestError> {
        if hex_hash.len() != Self::SHA256_HEX_LEN
            || !hex_hash.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(DigestError::InvalidFormat(format!("sha256:{hex_hash}")));
        }
        Ok(Self(format!("sha256:{hex_hash}")))
    }

    /// Returns the algorithm part of the digest (e.g., "sha256")
    pub fn algorithm(&self) -> &str {
        // Safe: Digest is only constructed via FromStr or sha256(), both of which validate format
        self.0.split(':').next().unwrap_or("sha256")
    }

    /// Returns the hex-encoded hash part of the digest
    pub fn hex_hash(&self) -> &str {
        // Safe: Digest is only constructed via FromStr or sha256(), both of which validate format
        self.0.split(':').nth(1).unwrap_or("")
    }

    /// Returns the full digest string (e.g., "sha256:abc123...")
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // OCI digest format: algorithm:encoded
        // algorithm: [a-z0-9]+([+._-][a-z0-9]+)*
        // encoded: [a-zA-Z0-9=_-]+
        let (algorithm, encoded) = s
            .split_once(':')
            .ok_or_else(|| DigestError::InvalidFormat(s.to_owned()))?;

        // Validate algorithm and get expected hash length
        let expected_len = match algorithm {
            "sha256" => Self::SHA256_HEX_LEN,
            "sha512" => Self::SHA512_HEX_LEN,
            _ => return Err(DigestError::UnsupportedAlgorithm(algorithm.to_owned())),
        };
        if encoded.len() != expected_len || !encoded.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DigestError::InvalidFormat(s.to_owned()));
        }

        Ok(Self(s.to_owned()))
    }
}

/// A tag name (e.g., "latest", "v1.0.0")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag(String);

#[derive(Debug, Error)]
pub enum TagError {
    #[error("Invalid tag: {0}")]
    Invalid(String),
}

impl Tag {
    /// Returns the tag as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validates a tag according to OCI spec
    fn is_valid(tag: &str) -> bool {
        // Tag: [a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}
        if tag.is_empty() || tag.len() > 128 {
            return false;
        }

        let mut chars = tag.chars();
        // Safe: we checked the tag is not empty above
        let Some(first) = chars.next() else {
            return false;
        };
        if !first.is_ascii_alphanumeric() && first != '_' {
            return false;
        }

        chars.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Tag {
    type Err = TagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if Self::is_valid(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(TagError::Invalid(s.to_owned()))
        }
    }
}

/// A reference to a manifest (either a tag or a digest)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    Tag(Tag),
    Digest(Digest),
}

impl Reference {
    /// Returns true if this reference is a digest
    pub fn is_digest(&self) -> bool {
        matches!(self, Self::Digest(_))
    }

    /// Returns true if this reference is a tag
    pub fn is_tag(&self) -> bool {
        matches!(self, Self::Tag(_))
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag(tag) => write!(f, "{tag}"),
            Self::Digest(digest) => write!(f, "{digest}"),
        }
    }
}

impl FromStr for Reference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // If it contains a colon and looks like a digest, parse as digest
        if s.contains(':')
            && let Ok(digest) = s.parse::<Digest>()
        {
            return Ok(Self::Digest(digest));
        }

        // Otherwise, try to parse as a tag
        s.parse::<Tag>().map(Self::Tag).map_err(|e| e.to_string())
    }
}

/// A unique identifier for an in-progress upload
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UploadId(String);

impl UploadId {
    /// Creates a new random upload ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the upload ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for UploadId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UploadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UploadId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Validate it's a valid UUID
        let _uuid: uuid::Uuid = s.parse()?;
        Ok(Self(s.to_owned()))
    }
}

/// Extracts the subject digest from manifest JSON bytes.
///
/// Returns `Some(subject_digest)` if the manifest has a `subject.digest` field
/// that parses as a valid `Digest`.
pub(crate) fn extract_subject_digest(manifest_bytes: &[u8]) -> Option<Digest> {
    let manifest = serde_json::from_slice::<serde_json::Value>(manifest_bytes).ok()?;
    let subject_digest_str = manifest.get("subject")?.get("digest")?.as_str()?;
    subject_digest_str.parse::<Digest>().ok()
}

/// Builds a referrer descriptor JSON from an already-parsed manifest.
///
/// Uses the typed `Manifest` fields (`media_type`, `artifact_type`, `annotations`,
/// `subject`) combined with the provided `digest` and `content_size` to produce an OCI
/// descriptor suitable for the referrers API.
///
/// Returns `None` if the manifest has no `subject` field.
pub(crate) fn build_referrer_descriptor(
    manifest: &bencher_json::oci::Manifest,
    digest: &Digest,
    content_size: usize,
) -> Option<(Digest, serde_json::Value)> {
    let subject = manifest.subject()?;
    let subject_digest = subject.digest.parse::<Digest>().ok()?;

    let media_type = manifest.media_type();
    let artifact_type = manifest.artifact_type();

    let mut descriptor = serde_json::json!({
        "mediaType": media_type,
        "digest": digest.to_string(),
        "size": content_size
    });
    if let Some(at) = artifact_type
        && let Some(obj) = descriptor.as_object_mut()
    {
        obj.insert(
            "artifactType".to_owned(),
            serde_json::Value::String(at.to_owned()),
        );
    }
    if let Some(annotations) = manifest.annotations()
        && let Some(obj) = descriptor.as_object_mut()
    {
        obj.insert(
            "annotations".to_owned(),
            serde_json::to_value(annotations).ok()?,
        );
    }

    Some((subject_digest, descriptor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_parsing() {
        let digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let parsed: Digest = digest.parse().unwrap();
        assert_eq!(parsed.algorithm(), "sha256");
        assert_eq!(
            parsed.hex_hash(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn tag_parsing() {
        assert!("latest".parse::<Tag>().is_ok());
        assert!("v1.0.0".parse::<Tag>().is_ok());
        assert!("1.0".parse::<Tag>().is_ok());
        assert!("_underscore".parse::<Tag>().is_ok());

        assert!("".parse::<Tag>().is_err());
        assert!("-dash-first".parse::<Tag>().is_err());
    }

    #[test]
    fn reference_parsing() {
        let tag_ref: Reference = "latest".parse().unwrap();
        assert!(tag_ref.is_tag());

        let digest_ref: Reference =
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .parse()
                .unwrap();
        assert!(digest_ref.is_digest());
    }

    #[test]
    fn digest_sha256_valid() {
        let digest =
            Digest::sha256("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();
        assert_eq!(digest.algorithm(), "sha256");
        assert_eq!(
            digest.hex_hash(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn digest_sha256_invalid() {
        assert!(Digest::sha256("").is_err());
        assert!(Digest::sha256("not-hex!").is_err());
        assert!(Digest::sha256("ZZZZ").is_err());
        // Too short
        assert!(Digest::sha256("abc123").is_err());
        // Too long (65 chars)
        assert!(
            Digest::sha256("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8550")
                .is_err()
        );
    }

    #[test]
    fn digest_sha512_valid() {
        let hash = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";
        let digest: Digest = format!("sha512:{hash}").parse().unwrap();
        assert_eq!(digest.algorithm(), "sha512");
        assert_eq!(digest.hex_hash(), hash);
    }

    #[test]
    fn digest_sha512_invalid_length() {
        // Too short
        assert!("sha512:abc123".parse::<Digest>().is_err());
        // sha256-length hash with sha512 algorithm
        assert!(
            "sha512:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .parse::<Digest>()
                .is_err()
        );
    }

    #[test]
    fn digest_from_str_rejects_short_sha256() {
        assert!("sha256:abc".parse::<Digest>().is_err());
    }

    #[test]
    fn upload_id() {
        let id = UploadId::new();
        let parsed: UploadId = id.as_str().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
