use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};
use sha2::Digest as _;

use crate::ValidError;

const SHA256_HEX_LEN: usize = 64;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Sha256(String);

impl Sha256 {
    pub fn compute(data: &[u8]) -> Self {
        let hash = sha2::Sha256::digest(data);
        Self(hex::encode(hash))
    }
}

impl FromStr for Sha256 {
    type Err = ValidError;

    fn from_str(hex_str: &str) -> Result<Self, Self::Err> {
        if is_valid_sha256(hex_str) {
            Ok(Self(hex_str.into()))
        } else {
            Err(ValidError::Sha256(hex_str.into()))
        }
    }
}

impl AsRef<str> for Sha256 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Sha256> for String {
    fn from(sha256: Sha256) -> Self {
        sha256.0
    }
}

impl<'de> Deserialize<'de> for Sha256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Sha256Visitor)
    }
}

struct Sha256Visitor;

impl Visitor<'_> for Sha256Visitor {
    type Value = Sha256;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a 64-character lowercase hex SHA-256 hash")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

pub fn is_valid_sha256(hex_str: &str) -> bool {
    hex_str.len() == SHA256_HEX_LEN && hex_str.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Sha256, is_valid_sha256};

    const VALID_HEX: &str = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";

    #[test]
    fn valid() {
        assert_eq!(true, is_valid_sha256(VALID_HEX));
        assert_eq!(
            true,
            is_valid_sha256("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
    }

    #[test]
    fn invalid() {
        for hex_str in [
            "",
            "abc",
            // 63 chars
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
            // 65 chars
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeff",
            // Uppercase
            "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            // Invalid chars
            "ghijklmnopqrstuvwxyz0123456789abcdef0123456789abcdef0123456789ab",
        ] {
            assert_eq!(false, is_valid_sha256(hex_str), "{hex_str}");
        }
    }

    #[test]
    fn from_str_valid() {
        let sha: Sha256 = VALID_HEX.parse().unwrap();
        assert_eq!(sha.as_ref(), VALID_HEX);
    }

    #[test]
    fn from_str_invalid() {
        assert!("invalid".parse::<Sha256>().is_err());
    }

    #[test]
    fn compute_and_display() {
        let sha = Sha256::compute(b"hello");
        assert_eq!(sha.as_ref().len(), 64);
        assert!(is_valid_sha256(sha.as_ref()));
    }

    #[test]
    fn serde_roundtrip() {
        let sha: Sha256 = VALID_HEX.parse().unwrap();
        let json = serde_json::to_string(&sha).unwrap();
        let parsed: Sha256 = serde_json::from_str(&json).unwrap();
        assert_eq!(sha, parsed);
    }
}
