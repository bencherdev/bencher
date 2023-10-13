use ::gix_hash::ObjectId;
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct GitHash(String);

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

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for GitHash
where
    DB: diesel::backend::Backend,
    for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        out.set_value(self.to_string());
        Ok(diesel::serialize::IsNull::No)
    }
}

#[cfg(feature = "db")]
impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for GitHash
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?.as_str().parse()?))
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_git_hash(git_hash: &str) -> bool {
    ObjectId::from_hex(git_hash.as_bytes()).is_ok()
}

#[cfg(test)]
mod test {
    use super::is_valid_git_hash;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_git_hash() {
        assert_eq!(
            true,
            is_valid_git_hash("1234567890ABCDEFAAAAAAAAAAAAAAAAAAAAAAAA")
        );
        assert_eq!(
            true,
            is_valid_git_hash("1234567890abcdefaaaaaaaaaaaaaaaaaaaaaaaa")
        );

        assert_eq!(false, is_valid_git_hash(""));
        assert_eq!(false, is_valid_git_hash("abcd"));
        assert_eq!(
            false,
            is_valid_git_hash("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaf")
        );
        assert_eq!(
            false,
            is_valid_git_hash("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
        );
    }
}
