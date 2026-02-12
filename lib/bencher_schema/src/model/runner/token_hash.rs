//! `TokenHash` newtype for SHA-256 hashed runner tokens stored in the database.

use std::{fmt, str::FromStr};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
};

/// A SHA-256 hashed runner token stored as TEXT in `SQLite`.
///
/// Provides type safety for token hashes while storing them as hex strings in the database.
#[derive(Debug, Clone, PartialEq, Eq, Hash, diesel::AsExpression, diesel::FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct TokenHash(String);

impl TokenHash {
    /// Create a new `TokenHash` from a hex-encoded hash string.
    pub fn new(hash: String) -> Self {
        Self(hash)
    }
}

impl fmt::Display for TokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TokenHash {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for TokenHash {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
    fn roundtrip() {
        let hash = TokenHash::new("abc123def456".to_owned());
        assert_eq!(hash.to_string(), "abc123def456");
        assert_eq!("abc123def456".parse::<TokenHash>().unwrap(), hash);
    }

    #[test]
    fn from_string() {
        let hash = TokenHash::from("test_hash".to_owned());
        assert_eq!(hash.to_string(), "test_hash");
    }
}
