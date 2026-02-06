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
    /// Creates a new SHA-256 digest from the given hex-encoded hash
    pub fn sha256(hex_hash: &str) -> Self {
        Self(format!("sha256:{hex_hash}"))
    }

    /// Returns the algorithm part of the digest (e.g., "sha256")
    pub fn algorithm(&self) -> &str {
        // Safe: Digest is only constructed via FromStr which validates format
        self.0.split(':').next().unwrap_or("sha256")
    }

    /// Returns the hex-encoded hash part of the digest
    pub fn hex_hash(&self) -> &str {
        // Safe: Digest is only constructed via FromStr which validates format
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

        // Validate algorithm - only sha256 and sha512 are commonly used
        if algorithm != "sha256" && algorithm != "sha512" {
            return Err(DigestError::UnsupportedAlgorithm(algorithm.to_owned()));
        }

        // Validate encoded hash is hex
        if encoded.is_empty() || !encoded.chars().all(|c| c.is_ascii_hexdigit()) {
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
    fn upload_id() {
        let id = UploadId::new();
        let parsed: UploadId = id.as_str().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
