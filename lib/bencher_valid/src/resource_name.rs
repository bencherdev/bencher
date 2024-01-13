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

use crate::{is_valid_len, Slug, UserName, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct ResourceName(String);

#[cfg(feature = "db")]
crate::typed_string!(ResourceName);

impl FromStr for ResourceName {
    type Err = ValidError;

    fn from_str(resource_name: &str) -> Result<Self, Self::Err> {
        if is_valid_resource_name(resource_name) {
            Ok(Self(resource_name.into()))
        } else {
            Err(ValidError::ResourceName(resource_name.into()))
        }
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

impl<'de> Deserialize<'de> for ResourceName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ResourceNameVisitor)
    }
}

struct ResourceNameVisitor;

impl Visitor<'_> for ResourceNameVisitor {
    type Value = ResourceName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a resource name string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_resource_name(resource_name: &str) -> bool {
    is_valid_len(resource_name)
}

#[cfg(test)]
mod test {
    use super::is_valid_resource_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_resource_name() {
        assert_eq!(true, is_valid_resource_name("a"));
        assert_eq!(true, is_valid_resource_name("ab"));
        assert_eq!(true, is_valid_resource_name("abc"));
        assert_eq!(true, is_valid_resource_name("ABC"));
        assert_eq!(true, is_valid_resource_name("abc ~ABC!"));
        assert_eq!(true, is_valid_resource_name(crate::test::LEN_50_STR));

        assert_eq!(false, is_valid_resource_name(crate::test::LEN_0_STR));
        assert_eq!(false, is_valid_resource_name(crate::test::LEN_51_STR));
    }
}
