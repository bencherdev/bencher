use std::sync::LazyLock;

use derive_more::Display;
#[cfg(all(feature = "server", not(feature = "client")))]
use regex::Regex;
#[cfg(feature = "client")]
use regex_lite::Regex;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::{REGEX_ERROR, Slug, ValidError, is_valid_len};

#[expect(clippy::expect_used)]
static NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9A-Za-z ,\.\-']{1,64}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct UserName(String);

#[cfg(feature = "db")]
crate::typed_string!(UserName);

impl FromStr for UserName {
    type Err = ValidError;

    fn from_str(user_name: &str) -> Result<Self, Self::Err> {
        if is_valid_user_name(user_name) {
            Ok(Self(user_name.into()))
        } else {
            Err(ValidError::UserName(user_name.into()))
        }
    }
}

impl AsRef<str> for UserName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Slug> for UserName {
    fn from(slug: Slug) -> Self {
        Self(slug.to_string())
    }
}

impl From<UserName> for String {
    fn from(user_name: UserName) -> Self {
        user_name.0
    }
}

impl<'de> Deserialize<'de> for UserName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UserNameVisitor)
    }
}

struct UserNameVisitor;

impl Visitor<'_> for UserNameVisitor {
    type Value = UserName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid user name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

impl UserName {
    pub const MAX_LEN: usize = crate::MAX_LEN;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_user_name(name: &str) -> bool {
    if !is_valid_len(name) {
        return false;
    }

    NAME_REGEX.is_match(name)
}

#[cfg(test)]
mod test {
    use crate::test::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::is_valid_user_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_is_valid_user_name() {
        assert_eq!(true, is_valid_user_name("muriel"));
        assert_eq!(true, is_valid_user_name("Muriel"));
        assert_eq!(true, is_valid_user_name("Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel    Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda Bagge"));
        assert_eq!(true, is_valid_user_name("Bagge, Muriel"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda-Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel De'Bagge"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Linda-De'Bagge"));
        assert_eq!(true, is_valid_user_name(LEN_64_STR));

        assert_eq!(false, is_valid_user_name(LEN_0_STR));
        assert_eq!(false, is_valid_user_name(LEN_65_STR));
        assert_eq!(false, is_valid_user_name(" Muriel Bagge"));
        assert_eq!(false, is_valid_user_name("Muriel Bagge "));
        assert_eq!(false, is_valid_user_name(" Muriel Bagge "));
        assert_eq!(false, is_valid_user_name("Muriel!"));
        assert_eq!(false, is_valid_user_name("Muriel! Bagge"));
    }
}
