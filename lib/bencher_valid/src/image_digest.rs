use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

/// An OCI image digest in the format `algorithm:hex`.
///
/// Supports:
/// - `sha256:` followed by 64 hex characters
/// - `sha512:` followed by 128 hex characters
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct ImageDigest(String);

#[cfg(feature = "db")]
crate::typed_string!(ImageDigest);

impl FromStr for ImageDigest {
    type Err = ValidError;

    fn from_str(digest: &str) -> Result<Self, Self::Err> {
        if is_valid_image_digest(digest) {
            Ok(Self(digest.into()))
        } else {
            Err(ValidError::ImageDigest(digest.into()))
        }
    }
}

impl ImageDigest {
    /// Return the algorithm portion of the digest (e.g. `"sha256"` or `"sha512"`).
    #[must_use]
    pub fn algorithm(&self) -> &str {
        self.0.split_once(':').map_or("sha256", |(alg, _)| alg)
    }

    /// Return the hex hash portion of the digest (after the `:`).
    #[must_use]
    pub fn hex_hash(&self) -> &str {
        self.0.split_once(':').map_or("", |(_, hex)| hex)
    }
}

impl AsRef<str> for ImageDigest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ImageDigest> for String {
    fn from(digest: ImageDigest) -> Self {
        digest.0
    }
}

impl<'de> Deserialize<'de> for ImageDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ImageDigestVisitor)
    }
}

struct ImageDigestVisitor;

impl Visitor<'_> for ImageDigestVisitor {
    type Value = ImageDigest;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid OCI image digest (sha256:... or sha512:...)")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

/// Validates an OCI image digest.
///
/// Valid formats:
/// - `sha256:` followed by exactly 64 lowercase hex characters
/// - `sha512:` followed by exactly 128 lowercase hex characters
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_image_digest(digest: &str) -> bool {
    if let Some(hex) = digest.strip_prefix("sha256:") {
        hex.len() == 64 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    } else if let Some(hex) = digest.strip_prefix("sha512:") {
        hex.len() == 128 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_image_digest;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_sha256_digest() {
        // Valid sha256 digests (64 hex chars)
        for digest in [
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
        ] {
            assert_eq!(true, is_valid_image_digest(digest), "{digest}");
        }
    }

    #[test]
    fn is_valid_sha512_digest() {
        // Valid sha512 digests (128 hex chars)
        let sha512 = format!("sha512:{}", "a".repeat(128));
        assert_eq!(true, is_valid_image_digest(&sha512));

        // 16 chars repeated 8 times = 128 chars
        let sha512_proper = format!("sha512:{}", "0123456789abcdef".repeat(8));
        assert_eq!(true, is_valid_image_digest(&sha512_proper));
    }

    #[test]
    fn is_invalid_digest() {
        for digest in [
            "",
            "sha256:",
            "sha512:",
            // Wrong length
            "sha256:abc",
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde", // 63 chars
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeff", // 65 chars
            // Invalid characters
            "sha256:ghijklmnopqrstuvwxyz0123456789abcdef0123456789abcdef0123456789", // 'g' is invalid
            // Uppercase hex (OCI spec requires lowercase)
            "sha256:ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            // Wrong prefix
            "sha384:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "md5:0123456789abcdef0123456789abcdef",
            // No prefix
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ] {
            assert_eq!(false, is_valid_image_digest(digest), "{digest}");
        }
    }

    #[test]
    fn parse_valid_digest() {
        let digest: super::ImageDigest =
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        assert_eq!(
            digest.as_ref(),
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn parse_invalid_digest() {
        let result: Result<super::ImageDigest, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn algorithm_sha256() {
        let digest: super::ImageDigest =
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        assert_eq!(digest.algorithm(), "sha256");
    }

    #[test]
    fn algorithm_sha512() {
        let digest: super::ImageDigest = format!("sha512:{}", "a".repeat(128)).parse().unwrap();
        assert_eq!(digest.algorithm(), "sha512");
    }

    #[test]
    fn hex_hash_sha256() {
        let digest: super::ImageDigest =
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        assert_eq!(
            digest.hex_hash(),
            "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn hex_hash_sha512() {
        let hex = "a".repeat(128);
        let digest: super::ImageDigest = format!("sha512:{hex}").parse().unwrap();
        assert_eq!(digest.hex_hash(), hex);
    }

    #[test]
    fn serde_roundtrip() {
        let digest: super::ImageDigest =
            "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        let json = serde_json::to_string(&digest).unwrap();
        let parsed: super::ImageDigest = serde_json::from_str(&json).unwrap();
        assert_eq!(digest, parsed);
    }
}
