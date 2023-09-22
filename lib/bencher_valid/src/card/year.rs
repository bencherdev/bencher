use chrono::Datelike;
use chrono::Utc;
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ExpirationYear(i32);

impl TryFrom<i64> for ExpirationYear {
    type Error = ValidError;

    fn try_from(expiration_year: i64) -> Result<Self, Self::Error> {
        Self::try_from(i32::try_from(expiration_year)?)
    }
}

impl TryFrom<i32> for ExpirationYear {
    type Error = ValidError;

    fn try_from(expiration_year: i32) -> Result<Self, Self::Error> {
        if is_valid_expiration_year(expiration_year) {
            Ok(Self(expiration_year))
        } else {
            Err(ValidError::ExpirationYear(expiration_year))
        }
    }
}

impl FromStr for ExpirationYear {
    type Err = ValidError;

    fn from_str(expiration_year: &str) -> Result<Self, Self::Err> {
        #[allow(clippy::map_err_ignore)]
        expiration_year
            .parse::<i32>()
            .map_err(|_| ValidError::ExpirationYearStr(expiration_year.into()))?
            .try_into()
    }
}

impl From<ExpirationYear> for i32 {
    fn from(expiration_year: ExpirationYear) -> Self {
        expiration_year.0
    }
}

impl<'de> Deserialize<'de> for ExpirationYear {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i32(ExpirationYearVisitor)
    }
}

struct ExpirationYearVisitor;

impl<'de> Visitor<'de> for ExpirationYearVisitor {
    type Value = ExpirationYear;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid payment card expiration year")
    }

    fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_i32(i32::try_from(value).map_err(E::custom)?)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_i32(i32::try_from(value).map_err(E::custom)?)
    }
}

// https://stackoverflow.com/questions/2500588/maximum-year-in-expiry-date-of-credit-card
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_expiration_year(year: i32) -> bool {
    let year_now = Utc::now().year();
    (year_now..=(year_now + 115)).contains(&year)
}

#[cfg(test)]
mod test {
    use super::is_valid_expiration_year;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_expiration_year() {
        assert_eq!(true, is_valid_expiration_year(2030));
        assert_eq!(true, is_valid_expiration_year(2040));
        assert_eq!(true, is_valid_expiration_year(2050));
        assert_eq!(true, is_valid_expiration_year(2060));

        assert_eq!(false, is_valid_expiration_year(-2030));
        assert_eq!(false, is_valid_expiration_year(0));
        assert_eq!(false, is_valid_expiration_year(2022));
    }
}
