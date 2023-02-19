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
static NUMBER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[[:digit:]]{12,19}$").expect(REGEX_ERROR));

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CardNumber(String);

impl FromStr for CardNumber {
    type Err = ValidError;

    fn from_str(card_number: &str) -> Result<Self, Self::Err> {
        if is_valid_card_number(card_number) {
            Ok(Self(card_number.into()))
        } else {
            Err(ValidError::CardNumber(card_number.into()))
        }
    }
}

impl AsRef<str> for CardNumber {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<CardNumber> for String {
    fn from(card_number: CardNumber) -> Self {
        card_number.0
    }
}

impl<'de> Deserialize<'de> for CardNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NonEmptyVisitor)
    }
}

struct NonEmptyVisitor;

impl<'de> Visitor<'de> for NonEmptyVisitor {
    type Value = CardNumber;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card number")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_card_number(card_number: &str) -> bool {
    NUMBER_REGEX.is_match(card_number) && validate_luhn(card_number)
}

pub fn validate_luhn(number: &str) -> bool {
    let mut checksum = 0;

    let mut iter = number.chars().rev();
    while let Some(c) = iter.next() {
        checksum += checksum_modifier_odd(c);
        if let Some(c) = iter.next() {
            checksum += checksum_modifier_even(c)
        }
    }

    checksum % 10 == 0
}

fn checksum_modifier_odd(c: char) -> u32 {
    numeric_char_to_u32(c)
}

fn checksum_modifier_even(c: char) -> u32 {
    let n = numeric_char_to_u32(c);
    let d = n * 2;
    if d <= 9 {
        d
    } else {
        d - 9
    }
}

fn numeric_char_to_u32(c: char) -> u32 {
    (c as u32) - ('0' as u32)
}

#[cfg(test)]
mod test {
    use super::is_valid_card_number;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_card_number() {
        let valid_numbers = vec![
            // visa electron
            "4917300800000000",
            // maestro
            "6759649826438453",
            // forbrugsforeningen
            "6007220000000004",
            // dankort
            "5019717010103742",
            // visa
            "4539571147647251",
            "4532983409238819",
            "4485600412608021",
            "4916252910064718",
            "4916738103790259",
            // amex
            "343380440754432",
            "377156543570043",
            "340173808718013",
            "375801706141502",
            "372728319416034",
            // mastercard
            "5236313877109142",
            "5431604665471808",
            "5571788302926264",
            "5411516521560216",
            "5320524083396284",
            // discover
            "6011297718292606",
            "6011993225918523",
            "6011420510510997",
            "6011618637473995",
            "6011786207277235",
            // jcb
            "3530111333300000",
            "3566002020360505",
            // union pay
            "6271136264806203568",
            "6236265930072952775",
            "6204679475679144515",
            "6216657720782466507",
            // diners club
            "30569309025904",
            "38520000023237",
            "36700102000000",
            "36148900647913",
        ];

        for valid_number in valid_numbers {
            assert_eq!(true, is_valid_card_number(valid_number));
        }

        let invalid_numbers = vec![
            "",
            "zduhehiudIHZHIUZHUI",
            "0292DYYEFYFEFYEFEFIUH",
            "00002837743671762",
            "1136283774",
            "424242424",
            "4242424242424244242424242",
            "523631387710914",
            // invalid luhn
            "5236313877109141",
            "6011420510510995",
        ];

        for invalid_number in invalid_numbers {
            assert_eq!(false, is_valid_card_number(invalid_number));
        }
    }
}
