use std::sync::LazyLock;

#[cfg(all(feature = "server", not(feature = "client")))]
use regex::Regex;
#[cfg(feature = "client")]
use regex_lite::Regex;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{ValidError, error::REGEX_ERROR, secret::SANITIZED_SECRET};

#[expect(clippy::expect_used)]
static CVC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[[:digit:]]{3,4}$").expect(REGEX_ERROR));

#[typeshare::typeshare]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct CardCvc(String);

impl TryFrom<String> for CardCvc {
    type Error = ValidError;

    fn try_from(card_cvc: String) -> Result<Self, Self::Error> {
        if is_valid_card_cvc(&card_cvc) {
            Ok(Self(card_cvc))
        } else {
            Err(ValidError::CardCvc(card_cvc))
        }
    }
}

impl FromStr for CardCvc {
    type Err = ValidError;

    fn from_str(card_cvc: &str) -> Result<Self, Self::Err> {
        Self::try_from(card_cvc.to_owned())
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

impl fmt::Display for CardCvc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(debug_assertions) {
            write!(f, "{}", self.0)
        } else {
            write!(f, "{SANITIZED_SECRET}",)
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_card_cvc(card_cvc: &str) -> bool {
    CVC_REGEX.is_match(card_cvc)
}

#[cfg(test)]
mod tests {
    use super::is_valid_card_cvc;
    use pretty_assertions::assert_eq;

    #[test]
    fn card_cvc() {
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
