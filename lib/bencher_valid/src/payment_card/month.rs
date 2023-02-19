use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ExpirationMonth(i32);

impl TryFrom<i32> for ExpirationMonth {
    type Error = ValidError;

    fn try_from(expiration_month: i32) -> Result<Self, Self::Error> {
        if is_valid_expiration_month(expiration_month) {
            Ok(Self(expiration_month))
        } else {
            Err(ValidError::ExpirationMonth(expiration_month))
        }
    }
}

impl From<ExpirationMonth> for i32 {
    fn from(expiration_month: ExpirationMonth) -> Self {
        expiration_month.0
    }
}

impl<'de> Deserialize<'de> for ExpirationMonth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i32(ExpirationMonthVisitor)
    }
}

struct ExpirationMonthVisitor;

impl<'de> Visitor<'de> for ExpirationMonthVisitor {
    type Value = ExpirationMonth;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card expiration month")
    }

    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_expiration_month(month: i32) -> bool {
    (1..13).contains(&month)
}

#[cfg(test)]
mod test {
    use super::is_valid_expiration_month;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_expiration_month() {
        for month in 1..13 {
            assert_eq!(true, is_valid_expiration_month(month));
        }

        assert_eq!(false, is_valid_expiration_month(-12));
        assert_eq!(false, is_valid_expiration_month(-1));
        assert_eq!(false, is_valid_expiration_month(0));
        assert_eq!(false, is_valid_expiration_month(13));
    }
}
