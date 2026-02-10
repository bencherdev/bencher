use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{Slug, UserName, ValidError, is_valid_len};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct ResourceName(String);

#[cfg(feature = "db")]
crate::typed_string!(ResourceName);

impl TryFrom<String> for ResourceName {
    type Error = ValidError;

    fn try_from(resource_name: String) -> Result<Self, Self::Error> {
        if is_valid_resource_name(&resource_name) {
            Ok(Self(resource_name))
        } else {
            Err(ValidError::ResourceName(resource_name))
        }
    }
}

impl FromStr for ResourceName {
    type Err = ValidError;

    fn from_str(resource_name: &str) -> Result<Self, Self::Err> {
        Self::try_from(resource_name.to_owned())
    }
}

impl AsRef<str> for ResourceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ResourceName> for String {
    fn from(resource_name: ResourceName) -> Self {
        resource_name.0
    }
}

impl From<UserName> for ResourceName {
    fn from(user_name: UserName) -> Self {
        Self(user_name.into())
    }
}

impl From<Slug> for ResourceName {
    fn from(slug: Slug) -> Self {
        Self(slug.into())
    }
}

impl ResourceName {
    pub const MAX_LEN: usize = crate::MAX_LEN;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_resource_name(resource_name: &str) -> bool {
    is_valid_len(resource_name)
}

#[cfg(test)]
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::is_valid_resource_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_resource_name_true() {
        for value in ["a", "ab", "abc", "ABC", "abc ~ABC!", LEN_64_STR] {
            assert_eq!(true, is_valid_resource_name(value), "{value}");
        }
    }

    #[test]
    fn is_valid_resource_name_false() {
        for value in [LEN_0_STR, LEN_65_STR] {
            assert_eq!(false, is_valid_resource_name(value), "{value}");
        }
    }

    #[test]
    fn resource_name_serde_roundtrip() {
        use super::ResourceName;

        let name: ResourceName = serde_json::from_str("\"My Resource\"").unwrap();
        assert_eq!(name.as_ref(), "My Resource");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"My Resource\"");

        let err = serde_json::from_str::<ResourceName>("\"\"");
        assert!(err.is_err());
    }
}
