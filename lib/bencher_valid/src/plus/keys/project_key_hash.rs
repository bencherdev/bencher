use derive_more::Display;
use std::str::FromStr;

use crate::ValidError;

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct ProjectKeyHash(String);

#[cfg(feature = "db")]
crate::typed_string!(ProjectKeyHash);

impl From<&crate::ProjectKey> for ProjectKeyHash {
    fn from(key: &crate::ProjectKey) -> Self {
        Self(super::sha256_hex(key.as_ref().as_bytes()))
    }
}

impl FromStr for ProjectKeyHash {
    type Err = ValidError;

    fn from_str(hash: &str) -> Result<Self, Self::Err> {
        if super::is_valid_sha256_hex(hash) {
            Ok(Self(hash.to_owned()))
        } else {
            Err(ValidError::ProjectKeyHash(hash.to_owned()))
        }
    }
}

impl AsRef<str> for ProjectKeyHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const VALID_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn valid() {
        assert!(VALID_HEX.parse::<ProjectKeyHash>().is_ok());
        assert!(
            "a".repeat(super::super::SHA256_HEX_LEN)
                .parse::<ProjectKeyHash>()
                .is_ok()
        );
    }

    #[test]
    fn invalid() {
        assert!("".parse::<ProjectKeyHash>().is_err());
        assert!("abc123".parse::<ProjectKeyHash>().is_err());
        assert!(
            "g".repeat(super::super::SHA256_HEX_LEN)
                .parse::<ProjectKeyHash>()
                .is_err()
        );
    }

    #[test]
    fn roundtrip() {
        let hash: ProjectKeyHash = VALID_HEX.parse().unwrap();
        assert_eq!(hash.to_string(), VALID_HEX);
    }

    #[test]
    fn from_project_key() {
        let key: crate::ProjectKey = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap();
        let hash = ProjectKeyHash::from(&key);
        assert_eq!(hash.as_ref().len(), super::super::SHA256_HEX_LEN);
        assert_eq!(hash, ProjectKeyHash::from(&key));

        let other: crate::ProjectKey = "bencher_run_xY9mN2pQ7rS4tU8vW1zK5jL0fGhaB3"
            .parse()
            .unwrap();
        assert_ne!(hash, ProjectKeyHash::from(&other));
    }
}
