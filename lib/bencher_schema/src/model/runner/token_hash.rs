//! `TokenHash` newtype for SHA-256 hashed runner tokens stored in the database.

use std::{fmt, str::FromStr};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
};

/// Expected length of a SHA-256 hex-encoded hash string.
const SHA256_HEX_LEN: usize = 64;

/// A SHA-256 hashed runner token stored as TEXT in `SQLite`.
///
/// Provides type safety for token hashes while storing them as hex strings in the database.
/// Validates that the hash is exactly 64 hex characters (SHA-256 output).
#[derive(Debug, Clone, PartialEq, Eq, Hash, diesel::AsExpression, diesel::FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct TokenHash(String);

#[derive(Debug, thiserror::Error)]
pub enum TokenHashError {
    #[error("Invalid token hash length: expected {SHA256_HEX_LEN}, got {0}")]
    Length(usize),
    #[error("Invalid token hash: contains non-hex character")]
    NotHex,
}

impl TokenHash {
    /// Create a new `TokenHash` from a hex-encoded hash string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not exactly 64 hex characters.
    pub fn new(hash: &str) -> Result<Self, TokenHashError> {
        hash.parse()
    }

    /// Hash a token string with SHA-256 and encode the digest as hex.
    ///
    /// Internally uses `sha2::Sha256` and `hex::encode`, producing a
    /// guaranteed-valid 64-character hex string.
    pub fn encode(token: &str) -> Self {
        use sha2::{Digest as _, Sha256};
        let digest = Sha256::digest(token.as_bytes());
        Self(hex::encode(digest))
    }
}

impl fmt::Display for TokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TokenHash {
    type Err = TokenHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != SHA256_HEX_LEN {
            return Err(TokenHashError::Length(s.len()));
        }
        if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(TokenHashError::NotHex);
        }
        Ok(Self(s.to_owned()))
    }
}

impl ToSql<Text, Sqlite> for TokenHash {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.0.clone());
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Text, DB> for TokenHash
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, DB>>::from_sql(bytes)?;
        Ok(Self(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_sha256_hex() {
        let hex = "a".repeat(SHA256_HEX_LEN);
        let hash = hex.parse::<TokenHash>().unwrap();
        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn valid_mixed_hex() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let hash = hex.parse::<TokenHash>().unwrap();
        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn new_valid() {
        let hex = "f".repeat(SHA256_HEX_LEN);
        let hash = TokenHash::new(&hex).unwrap();
        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn wrong_length_short() {
        let result = "abc123".parse::<TokenHash>();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TokenHashError::Length(6)));
    }

    #[test]
    fn wrong_length_long() {
        let hex = "a".repeat(65);
        let result = hex.parse::<TokenHash>();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TokenHashError::Length(65)));
    }

    #[test]
    fn non_hex_characters() {
        let mut hex = "g".repeat(SHA256_HEX_LEN);
        hex.replace_range(0..1, "g");
        let result = hex.parse::<TokenHash>();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TokenHashError::NotHex));
    }

    #[test]
    fn empty_string() {
        let result = "".parse::<TokenHash>();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TokenHashError::Length(0)));
    }

    #[test]
    fn roundtrip() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let hash = TokenHash::new(hex).unwrap();
        assert_eq!(hash.to_string(), hex);
        assert_eq!(hex.parse::<TokenHash>().unwrap(), hash);
    }

    #[test]
    fn encode_token() {
        let hash = TokenHash::encode("test_token");
        // SHA-256 hex is always 64 chars
        assert_eq!(hash.to_string().len(), SHA256_HEX_LEN);
        // Deterministic
        assert_eq!(hash, TokenHash::encode("test_token"));
        // Different input → different hash
        assert_ne!(hash, TokenHash::encode("other_token"));
    }
}
