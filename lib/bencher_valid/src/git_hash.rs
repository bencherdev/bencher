use derive_more::Display;
use gix_hash::ObjectId;
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

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct GitHash(String);

#[cfg(feature = "db")]
crate::typed_string!(GitHash);

impl FromStr for GitHash {
    type Err = ValidError;

    fn from_str(git_hash: &str) -> Result<Self, Self::Err> {
        if is_valid_git_hash(git_hash) {
            Ok(Self(git_hash.into()))
        } else {
            Err(ValidError::GitHash(git_hash.into()))
        }
    }
}

impl AsRef<str> for GitHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<GitHash> for String {
    fn from(git_hash: GitHash) -> Self {
        git_hash.0
    }
}

impl From<ObjectId> for GitHash {
    fn from(object_id: ObjectId) -> Self {
        Self(object_id.to_string())
    }
}

impl<'de> Deserialize<'de> for GitHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(GitHashVisitor)
    }
}

struct GitHashVisitor;

impl Visitor<'_> for GitHashVisitor {
    type Value = GitHash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid git_hash")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_git_hash(git_hash: &str) -> bool {
    ObjectId::from_hex(git_hash.as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::is_valid_git_hash;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_git_hash_true() {
        for hash in [
            "1234567890ABCDEFAAAAAAAAAAAAAAAAAAAAAAAA",
            "1234567890abcdefaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert_eq!(true, is_valid_git_hash(hash), "{hash}");
        }
    }

    #[test]
    fn is_valid_git_hash_false() {
        for hash in [
            "",
            "abcd",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaf",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert_eq!(false, is_valid_git_hash(hash), "{hash}");
        }
    }
}
