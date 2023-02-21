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

const AMEX: &str = "amex";
const DINERS: &str = "diners";
const DISCOVER: &str = "discover";
const JCB: &str = "jcb";
const MASTERCARD: &str = "mastercard";
const UNIONPAY: &str = "unionpay";
const VISA: &str = "visa";
const UNKNOWN: &str = "unknown";

#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
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

impl FromStr for CardBrand {
    type Err = ValidError;

    fn from_str(card_brand: &str) -> Result<Self, Self::Err> {
        if is_valid_card_brand(card_brand) {
            return Ok(match card_brand {
                AMEX => Self::Amex,
                DINERS => Self::Diners,
                DISCOVER => Self::Discover,
                JCB => Self::Jcb,
                MASTERCARD => Self::Mastercard,
                UNIONPAY => Self::Unionpay,
                VISA => Self::Visa,
                UNKNOWN => Self::Unknown,
                _ => panic!("Invalid card brand"),
            });
        }

        Err(ValidError::CardBrand(card_brand.into()))
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
        card_brand.as_ref().to_string()
    }
}

impl<'de> Deserialize<'de> for CardBrand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CardBrandVisitor)
    }
}

struct CardBrandVisitor;

impl<'de> Visitor<'de> for CardBrandVisitor {
    type Value = CardBrand;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card brand")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
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
mod test {
    use super::is_valid_card_brand;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_card_brand() {
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
}
