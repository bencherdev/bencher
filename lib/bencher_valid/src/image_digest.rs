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

/// An OCI image digest (e.g., "sha256:abc123...")
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
        formatter.write_str("a valid OCI image digest (e.g., sha256:...)")
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
/// Valid format: `sha256:` followed by exactly 64 lowercase hexadecimal characters.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_image_digest(digest: &str) -> bool {
    let Some(hash) = digest.strip_prefix("sha256:") else {
        return false;
    };

    // SHA-256 produces 32 bytes = 64 hex characters
    if hash.len() != 64 {
        return false;
    }

    // Must be all lowercase hexadecimal
    hash.bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::is_valid_image_digest;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_image_digest_true() {
        for digest in [
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ] {
            assert_eq!(true, is_valid_image_digest(digest), "{digest}");
        }
    }

    #[test]
    fn is_valid_image_digest_false() {
        for digest in [
            "",
            "sha256:",
            "sha256:abc",
            "sha512:0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            // Uppercase not allowed
            "sha256:ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            // Mixed case not allowed
            "sha256:ABCDEf0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            // Too short
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85",
            // Too long
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8555",
            // Invalid characters
            "sha256:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert_eq!(false, is_valid_image_digest(digest), "{digest}");
        }
    }
}
