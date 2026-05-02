use derive_more::Display;
use sha2::Digest as _;
use std::str::FromStr;

use crate::ValidError;

const SHA256_HEX_LEN: usize = 64;

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct RunnerKeyHash(String);

#[cfg(feature = "db")]
crate::typed_string!(RunnerKeyHash);

impl From<&crate::RunnerKey> for RunnerKeyHash {
    fn from(key: &crate::RunnerKey) -> Self {
        let digest = sha2::Sha256::digest(key.as_ref().as_bytes());
        Self(hex::encode(digest))
    }
}

impl FromStr for RunnerKeyHash {
    type Err = ValidError;

    fn from_str(hash: &str) -> Result<Self, Self::Err> {
        if is_valid_runner_key_hash(hash) {
            Ok(Self(hash.to_owned()))
        } else {
            Err(ValidError::RunnerKeyHash(hash.to_owned()))
        }
    }
}

impl AsRef<str> for RunnerKeyHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn is_valid_runner_key_hash(hex_str: &str) -> bool {
    hex_str.len() == SHA256_HEX_LEN && hex_str.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const VALID_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn valid() {
        assert!(VALID_HEX.parse::<RunnerKeyHash>().is_ok());
        assert!("a".repeat(SHA256_HEX_LEN).parse::<RunnerKeyHash>().is_ok());
    }

    #[test]
    fn invalid() {
        assert!("".parse::<RunnerKeyHash>().is_err());
        assert!("abc123".parse::<RunnerKeyHash>().is_err());
        assert!("g".repeat(SHA256_HEX_LEN).parse::<RunnerKeyHash>().is_err());
    }

    #[test]
    fn roundtrip() {
        let hash: RunnerKeyHash = VALID_HEX.parse().unwrap();
        assert_eq!(hash.to_string(), VALID_HEX);
    }

    #[test]
    fn from_runner_key() {
        let key: crate::RunnerKey = "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap();
        let hash = RunnerKeyHash::from(&key);
        assert_eq!(hash.as_ref().len(), SHA256_HEX_LEN);
        assert_eq!(hash, RunnerKeyHash::from(&key));

        let other: crate::RunnerKey = "bencher_runner_xY9mN2pQ7rS4tU8vW1zK5jL0fGhaB3"
            .parse()
            .unwrap();
        assert_ne!(hash, RunnerKeyHash::from(&other));
    }
}
