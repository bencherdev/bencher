use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

const AMEX: &str = "amex";
const DINERS: &str = "diners";
const DISCOVER: &str = "discover";
const JCB: &str = "jcb";
const MASTERCARD: &str = "mastercard";
const UNIONPAY: &str = "unionpay";
const VISA: &str = "visa";
const UNKNOWN: &str = "unknown";

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String", rename_all = "snake_case")]
pub enum CardBrand {
    Amex,
    Diners,
    Discover,
    Jcb,
    Mastercard,
    Unionpay,
    Visa,
    #[default]
    Unknown,
}

impl TryFrom<String> for CardBrand {
    type Error = ValidError;

    fn try_from(card_brand: String) -> Result<Self, Self::Error> {
        match card_brand.as_str() {
            AMEX => Ok(Self::Amex),
            DINERS => Ok(Self::Diners),
            DISCOVER => Ok(Self::Discover),
            JCB => Ok(Self::Jcb),
            MASTERCARD => Ok(Self::Mastercard),
            UNIONPAY => Ok(Self::Unionpay),
            VISA => Ok(Self::Visa),
            UNKNOWN => Ok(Self::Unknown),
            _ => Err(ValidError::CardBrand(card_brand)),
        }
    }
}

impl FromStr for CardBrand {
    type Err = ValidError;

    fn from_str(card_brand: &str) -> Result<Self, Self::Err> {
        Self::try_from(card_brand.to_owned())
    }
}

impl AsRef<str> for CardBrand {
    fn as_ref(&self) -> &str {
        match self {
            CardBrand::Amex => AMEX,
            CardBrand::Diners => DINERS,
            CardBrand::Discover => DISCOVER,
            CardBrand::Jcb => JCB,
            CardBrand::Mastercard => MASTERCARD,
            CardBrand::Unionpay => UNIONPAY,
            CardBrand::Visa => VISA,
            CardBrand::Unknown => UNKNOWN,
        }
    }
}

impl From<CardBrand> for String {
    fn from(card_brand: CardBrand) -> Self {
        card_brand.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_card_brand(card_brand: &str) -> bool {
    matches!(
        card_brand,
        AMEX | DINERS | DISCOVER | JCB | MASTERCARD | UNIONPAY | VISA | UNKNOWN
    )
}

#[cfg(test)]
mod tests {
    use super::is_valid_card_brand;
    use pretty_assertions::assert_eq;

    #[test]
    fn card_brand() {
        assert_eq!(true, is_valid_card_brand("amex"));
        assert_eq!(true, is_valid_card_brand("diners"));
        assert_eq!(true, is_valid_card_brand("discover"));
        assert_eq!(true, is_valid_card_brand("jcb"));
        assert_eq!(true, is_valid_card_brand("mastercard"));
        assert_eq!(true, is_valid_card_brand("unionpay"));
        assert_eq!(true, is_valid_card_brand("visa"));
        assert_eq!(true, is_valid_card_brand("unknown"));

        assert_eq!(false, is_valid_card_brand(""));
        assert_eq!(false, is_valid_card_brand("one"));
        assert_eq!(false, is_valid_card_brand("two"));
        assert_eq!(false, is_valid_card_brand(" amex"));
        assert_eq!(false, is_valid_card_brand("amex "));
        assert_eq!(false, is_valid_card_brand(" amex "));
    }

    #[test]
    fn card_brand_serde_roundtrip() {
        use super::CardBrand;

        let brand: CardBrand = serde_json::from_str("\"amex\"").unwrap();
        assert_eq!(brand, CardBrand::Amex);
        let json = serde_json::to_string(&brand).unwrap();
        assert_eq!(json, "\"amex\"");

        let err = serde_json::from_str::<CardBrand>("\"invalid\"");
        assert!(err.is_err());
    }
}
