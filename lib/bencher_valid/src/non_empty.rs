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

#[derive(Debug, Display, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct NonEmpty(String);

impl FromStr for NonEmpty {
    type Err = ValidError;

    fn from_str(non_empty: &str) -> Result<Self, Self::Err> {
        if is_valid_non_empty(non_empty) {
            Ok(Self(non_empty.into()))
        } else {
            Err(ValidError::NonEmpty(non_empty.into()))
        }
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

impl<'de> Deserialize<'de> for NonEmpty {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NonEmptyVisitor)
    }
}

struct NonEmptyVisitor;

impl<'de> Visitor<'de> for NonEmptyVisitor {
    type Value = NonEmpty;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a non-empty string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_non_empty(non_empty: &str) -> bool {
    !non_empty.is_empty()
}

#[cfg(test)]
mod test {
    use super::is_valid_non_empty;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_non_empty() {
        assert_eq!(true, is_valid_non_empty("refs/heads/main"));
        assert_eq!(true, is_valid_non_empty("main"));
        assert_eq!(true, is_valid_non_empty("MAIN"));
        assert_eq!(true, is_valid_non_empty("bencher/main"));
        assert_eq!(true, is_valid_non_empty(" main"));
        assert_eq!(true, is_valid_non_empty("ma in"));
        assert_eq!(true, is_valid_non_empty("main "));
        assert_eq!(true, is_valid_non_empty(".main"));

        assert_eq!(false, is_valid_non_empty(""));
    }
}
