use derive_more::Display;
use std::str::FromStr;

use crate::ValidError;

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct UserKeyHash(String);

#[cfg(feature = "db")]
crate::typed_string!(UserKeyHash);

#[cfg(feature = "server")]
impl From<&crate::UserKey> for UserKeyHash {
    fn from(key: &crate::UserKey) -> Self {
        Self(super::sha256_hex(key.as_ref().as_bytes()))
    }
}

impl FromStr for UserKeyHash {
    type Err = ValidError;

    fn from_str(hash: &str) -> Result<Self, Self::Err> {
        if super::is_valid_sha256_hex(hash) {
            Ok(Self(hash.to_owned()))
        } else {
            Err(ValidError::UserKeyHash(hash.to_owned()))
        }
    }
}

impl AsRef<str> for UserKeyHash {
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
        VALID_HEX.parse::<UserKeyHash>().unwrap();
        "a".repeat(super::super::SHA256_HEX_LEN)
            .parse::<UserKeyHash>()
            .unwrap();
    }

    #[test]
    fn invalid() {
        "".parse::<UserKeyHash>().unwrap_err();
        "abc123".parse::<UserKeyHash>().unwrap_err();
        "g".repeat(super::super::SHA256_HEX_LEN)
            .parse::<UserKeyHash>()
            .unwrap_err();
    }

    #[test]
    fn roundtrip() {
        let hash: UserKeyHash = VALID_HEX.parse().unwrap();
        assert_eq!(hash.to_string(), VALID_HEX);
    }

    #[cfg(feature = "server")]
    #[test]
    fn from_user_key() {
        let key: crate::UserKey = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap();
        let hash = UserKeyHash::from(&key);
        assert_eq!(hash.as_ref().len(), super::super::SHA256_HEX_LEN);
        assert_eq!(hash, UserKeyHash::from(&key));

        let other: crate::UserKey = "bencher_user_xY9mN2pQ7rS4tU8vW1zK5jL0fGhaB3"
            .parse()
            .unwrap();
        assert_ne!(hash, UserKeyHash::from(&other));
    }
}
