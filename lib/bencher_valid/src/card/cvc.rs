use derive_more::Display;
use once_cell::sync::Lazy;
use regex::Regex;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{error::REGEX_ERROR, ValidError};

#[allow(clippy::expect_used)]
static CVC_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[[:digit:]]{3,4}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CardCvc(String);

impl FromStr for CardCvc {
    type Err = ValidError;

    fn from_str(card_cvc: &str) -> Result<Self, Self::Err> {
        if is_valid_card_cvc(card_cvc) {
            Ok(Self(card_cvc.into()))
        } else {
            Err(ValidError::CardCvc(card_cvc.into()))
        }
    }
}

impl AsRef<str> for CardCvc {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<CardCvc> for String {
    fn from(card_cvc: CardCvc) -> Self {
        card_cvc.0
    }
}

impl<'de> Deserialize<'de> for CardCvc {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CardCvcVisitor)
    }
}

struct CardCvcVisitor;

impl<'de> Visitor<'de> for CardCvcVisitor {
    type Value = CardCvc;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card CVC")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_card_cvc(card_cvc: &str) -> bool {
    CVC_REGEX.is_match(card_cvc)
}

#[cfg(test)]
mod test {
    use super::is_valid_card_cvc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_card_cvc() {
        assert_eq!(true, is_valid_card_cvc("012"));
        assert_eq!(true, is_valid_card_cvc("0123"));
        assert_eq!(true, is_valid_card_cvc("123"));
        assert_eq!(true, is_valid_card_cvc("1234"));

        assert_eq!(false, is_valid_card_cvc(""));
        assert_eq!(false, is_valid_card_cvc("0"));
        assert_eq!(false, is_valid_card_cvc("01234"));
        assert_eq!(false, is_valid_card_cvc("12345"));
        assert_eq!(false, is_valid_card_cvc("bad"));
    }
}
