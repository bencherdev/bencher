use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{UserName, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct NonEmpty(String);

#[cfg(feature = "db")]
crate::typed_string!(NonEmpty);

impl TryFrom<String> for NonEmpty {
    type Error = ValidError;

    fn try_from(non_empty: String) -> Result<Self, Self::Error> {
        if is_valid_non_empty(&non_empty) {
            Ok(Self(non_empty))
        } else {
            Err(ValidError::NonEmpty(non_empty))
        }
    }
}

impl FromStr for NonEmpty {
    type Err = ValidError;

    fn from_str(non_empty: &str) -> Result<Self, Self::Err> {
        Self::try_from(non_empty.to_owned())
    }
}

impl AsRef<str> for NonEmpty {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<NonEmpty> for String {
    fn from(non_empty: NonEmpty) -> Self {
        non_empty.0
    }
}

impl From<UserName> for NonEmpty {
    fn from(user_name: UserName) -> Self {
        Self(user_name.into())
    }
}

impl NonEmpty {
    pub const MAX_LEN: usize = crate::MAX_LEN;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_non_empty(non_empty: &str) -> bool {
    crate::is_valid_non_empty(non_empty)
}

#[cfg(test)]
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::is_valid_non_empty;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_non_empty_true() {
        for value in ["a", "ab", "abc", "ABC", "abc ~ABC!", LEN_64_STR, LEN_65_STR] {
            assert_eq!(true, is_valid_non_empty(value), "{value}");
        }
    }

    #[test]
    fn is_valid_non_empty_false() {
        assert_eq!(false, is_valid_non_empty(LEN_0_STR));
    }
}
