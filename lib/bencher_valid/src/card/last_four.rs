use derive_more::Display;
use once_cell::sync::Lazy;
#[cfg(not(feature = "wasm"))]
use regex::Regex;
#[cfg(feature = "wasm")]
use regex_lite::Regex;
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
static LAST_FOUR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[[:digit:]]{4}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct LastFour(String);

impl FromStr for LastFour {
    type Err = ValidError;

    fn from_str(last_four: &str) -> Result<Self, Self::Err> {
        if is_valid_last_four(last_four) {
            Ok(Self(last_four.into()))
        } else {
            Err(ValidError::LastFour(last_four.into()))
        }
    }
}

impl AsRef<str> for LastFour {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<LastFour> for String {
    fn from(last_four: LastFour) -> Self {
        last_four.0
    }
}

impl<'de> Deserialize<'de> for LastFour {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(LastFourVisitor)
    }
}

struct LastFourVisitor;

impl Visitor<'_> for LastFourVisitor {
    type Value = LastFour;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card last four numbers")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_last_four(last_four: &str) -> bool {
    LAST_FOUR_REGEX.is_match(last_four)
}

#[cfg(test)]
mod test {
    use super::is_valid_last_four;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_last_four() {
        let valid_numbers = vec![
            // visa electron
            "0000", // maestro
            "8453", // forbrugsforeningen
            "0004", // dankort
            "3742", // visa
            "7251", "8819", "8021", "4718", "0259", // amex
            "4432", "0043", "8013", "1502", "6034", // mastercard
            "9142", "1808", "6264", "0216", "6284", // discover
            "2606", "8523", "0997", "3995", "7235", // jcb
            "0000", "0505", // union pay
            "3568", "2775", "4515", "6507", // diners club
            "5904", "3237", "0000", "7913",
        ];

        for valid_number in valid_numbers {
            assert_eq!(true, is_valid_last_four(valid_number));
        }

        let invalid_numbers = vec!["", "XXXX", "000X", "00X0", "0X00", "X000"];

        for invalid_number in invalid_numbers {
            assert_eq!(false, is_valid_last_four(invalid_number));
        }
    }
}
