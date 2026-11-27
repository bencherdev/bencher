use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_CONFIRMATIONS: u32 = 1;
const MAX_CONFIRMATIONS: u32 = 20;

/// The number of consecutive scans that must find a changepoint before it is confirmed.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct Confirmations(u32);

impl Confirmations {
    /// The count a changepoint model takes when it names none.
    pub const DEFAULT: Self = Self(5);
}

impl TryFrom<u32> for Confirmations {
    type Error = ValidError;

    fn try_from(confirmations: u32) -> Result<Self, Self::Error> {
        is_valid_confirmations(confirmations)
            .then_some(Self(confirmations))
            .ok_or(ValidError::Confirmations(confirmations))
    }
}

impl From<Confirmations> for u32 {
    fn from(confirmations: Confirmations) -> Self {
        confirmations.0
    }
}

impl From<Confirmations> for i64 {
    fn from(confirmations: Confirmations) -> Self {
        Self::from(confirmations.0)
    }
}

impl FromStr for Confirmations {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(u32::from_str(s).map_err(ValidError::ConfirmationsStr)?)
    }
}

impl<'de> Deserialize<'de> for Confirmations {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(ConfirmationsVisitor)
    }
}

struct ConfirmationsVisitor;

impl Visitor<'_> for ConfirmationsVisitor {
    type Value = Confirmations;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a model `confirmations` count from 1 to 20")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u32::try_from(v)
            .map_err(|_err| ValidError::Confirmations(u32::MAX))
            .and_then(Confirmations::try_from)
            .map_err(E::custom)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::Confirmations;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Confirmations
    where
        DB: diesel::backend::Backend,
        for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i64::from(*self));
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Confirmations
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            u32::try_from(i64::from_sql(bytes)?)?
                .try_into()
                .map_err(Into::into)
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_confirmations(confirmations: u32) -> bool {
    (MIN_CONFIRMATIONS..=MAX_CONFIRMATIONS).contains(&confirmations)
}

#[cfg(test)]
mod tests {
    use super::Confirmations;

    #[test]
    fn confirmations_are_from_1_to_20() {
        for valid in [1, 5, 20] {
            Confirmations::try_from(valid).unwrap();
        }
        for invalid in [0, 21, u32::MAX] {
            Confirmations::try_from(invalid).unwrap_err();
        }
    }

    #[test]
    fn confirmations_serde_names_the_field() {
        let confirmations: Confirmations = serde_json::from_str("20").unwrap();
        assert_eq!(20, u32::from(confirmations));
        for invalid in ["0", "21", "-1", "1.5", "4294967296"] {
            let err = serde_json::from_str::<Confirmations>(invalid)
                .unwrap_err()
                .to_string();
            assert!(err.contains("`confirmations`"), "{invalid}: {err}");
        }
    }

    #[test]
    fn confirmations_from_str() {
        assert_eq!(3, u32::from("3".parse::<Confirmations>().unwrap()));
        "0".parse::<Confirmations>().unwrap_err();
        "three".parse::<Confirmations>().unwrap_err();
    }
}
