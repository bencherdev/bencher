use std::sync::LazyLock;

use derive_more::Display;
#[cfg(all(feature = "server", not(feature = "client")))]
use regex::Regex;
#[cfg(feature = "client")]
use regex_lite::Regex;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{REGEX_ERROR, Slug, ValidError, is_valid_len};

#[expect(clippy::expect_used)]
static NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9A-Za-z ,\.\-']{1,64}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct UserName(String);

#[cfg(feature = "db")]
crate::typed_string!(UserName);

impl TryFrom<String> for UserName {
    type Error = ValidError;

    fn try_from(user_name: String) -> Result<Self, Self::Error> {
        if is_valid_user_name(&user_name) {
            Ok(Self(user_name))
        } else {
            Err(ValidError::UserName(user_name))
        }
    }
}

impl FromStr for UserName {
    type Err = ValidError;

    fn from_str(user_name: &str) -> Result<Self, Self::Err> {
        Self::try_from(user_name.to_owned())
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
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::is_valid_user_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_user_name_true() {
        for name in [
            "muriel",
            "Muriel",
            "Muriel Bagge",
            "Muriel    Bagge",
            "Muriel Linda Bagge",
            "Bagge, Muriel",
            "Mrs. Muriel Bagge",
            "Muriel Linda-Bagge",
            "Muriel De'Bagge",
            "Mrs. Muriel Linda-De'Bagge",
            LEN_64_STR,
        ] {
            assert_eq!(true, is_valid_user_name(name), "{name}");
        }
    }

    #[test]
    fn is_valid_user_name_false() {
        for name in [
            LEN_0_STR,
            LEN_65_STR,
            " Muriel Bagge",
            "Muriel Bagge ",
            " Muriel Bagge ",
            "Muriel!",
            "Muriel! Bagge",
        ] {
            assert_eq!(false, is_valid_user_name(name), "{name}");
        }
    }
}
