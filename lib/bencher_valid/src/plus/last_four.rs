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

use crate::{ValidError, error::REGEX_ERROR};

#[expect(clippy::expect_used)]
static LAST_FOUR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[[:digit:]]{4}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct LastFour(String);

impl TryFrom<String> for LastFour {
    type Error = ValidError;

    fn try_from(last_four: String) -> Result<Self, Self::Error> {
        if is_valid_last_four(&last_four) {
            Ok(Self(last_four))
        } else {
            Err(ValidError::LastFour(last_four))
        }
    }
}

impl FromStr for LastFour {
    type Err = ValidError;

    fn from_str(last_four: &str) -> Result<Self, Self::Err> {
        Self::try_from(last_four.to_owned())
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

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_last_four(last_four: &str) -> bool {
    LAST_FOUR_REGEX.is_match(last_four)
}

#[cfg(test)]
mod tests {
    use super::is_valid_last_four;
    use pretty_assertions::assert_eq;

    #[test]
    fn last_four() {
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

    #[test]
    fn last_four_serde_roundtrip() {
        use super::LastFour;

        let last_four: LastFour = serde_json::from_str("\"1234\"").unwrap();
        assert_eq!(last_four.as_ref(), "1234");
        let json = serde_json::to_string(&last_four).unwrap();
        assert_eq!(json, "\"1234\"");

        let err = serde_json::from_str::<LastFour>("\"XXXX\"");
        assert!(err.is_err());
    }
}
