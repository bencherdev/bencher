use derive_more::Display;
use git_validate::reference::name_partial;
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
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct BranchName(String);

impl FromStr for BranchName {
    type Err = ValidError;

    fn from_str(branch_name: &str) -> Result<Self, Self::Err> {
        if is_valid_branch_name(branch_name) {
            Ok(Self(branch_name.into()))
        } else {
            Err(ValidError::BranchName(branch_name.into()))
        }
    }
}

impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<BranchName> for String {
    fn from(branch_name: BranchName) -> Self {
        branch_name.0
    }
}

impl<'de> Deserialize<'de> for BranchName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BranchNameVisitor)
    }
}

struct BranchNameVisitor;

impl Visitor<'_> for BranchNameVisitor {
    type Value = BranchName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid branch name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for BranchName
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
impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for BranchName
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?.as_str().parse()?))
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_branch_name(branch_name: &str) -> bool {
    name_partial(branch_name.into()).is_ok()
}

#[cfg(test)]
mod test {
    use super::is_valid_branch_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_branch_name() {
        assert_eq!(true, is_valid_branch_name("refs/heads/main"));
        assert_eq!(true, is_valid_branch_name("main"));
        assert_eq!(true, is_valid_branch_name("MAIN"));
        assert_eq!(true, is_valid_branch_name("bencher/main"));

        assert_eq!(false, is_valid_branch_name(""));
        assert_eq!(false, is_valid_branch_name(" main"));
        assert_eq!(false, is_valid_branch_name("ma in"));
        assert_eq!(false, is_valid_branch_name("main "));
        assert_eq!(false, is_valid_branch_name(".main"));
    }
}
