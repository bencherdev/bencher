use derive_more::Display;
use ordered_float::OrderedFloat;
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

/// The chance of a false alarm a test may take, for example `0.001`.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Double))]
pub struct Significance(OrderedFloat<f64>);

impl TryFrom<f64> for Significance {
    type Error = ValidError;

    fn try_from(significance: f64) -> Result<Self, Self::Error> {
        is_valid_significance(significance)
            .then(|| Self(significance.into()))
            .ok_or(ValidError::Significance(significance))
    }
}

impl From<Significance> for f64 {
    fn from(significance: Significance) -> Self {
        significance.0.into()
    }
}

impl FromStr for Significance {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(f64::from_str(s).map_err(ValidError::SignificanceStr)?)
    }
}

impl<'de> Deserialize<'de> for Significance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_f64(SignificanceVisitor)
    }
}

struct SignificanceVisitor;

impl Visitor<'_> for SignificanceVisitor {
    type Value = Significance;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a model `significance` above 0 and below 1")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::invalid_value(de::Unexpected::Unsigned(v), &self))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::invalid_value(de::Unexpected::Signed(v), &self))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::Significance;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Double, DB> for Significance
    where
        DB: diesel::backend::Backend,
        for<'a> f64: diesel::serialize::ToSql<diesel::sql_types::Double, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(f64::from(*self));
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Double, DB> for Significance
    where
        DB: diesel::backend::Backend,
        f64: diesel::deserialize::FromSql<diesel::sql_types::Double, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            f64::from_sql(bytes)?.try_into().map_err(Into::into)
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_significance(significance: f64) -> bool {
    significance > 0.0 && significance < 1.0
}

#[cfg(test)]
mod tests {
    use super::Significance;

    #[test]
    fn significance_is_above_zero_and_below_one() {
        for valid in [f64::MIN_POSITIVE, 0.001, 0.05, 0.5, 1.0 - f64::EPSILON] {
            Significance::try_from(valid).unwrap();
        }
        for invalid in [
            0.0,
            -0.0,
            -0.001,
            1.0,
            1.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            Significance::try_from(invalid).unwrap_err();
        }
    }

    #[test]
    fn significance_serde_names_the_field() {
        let significance: Significance = serde_json::from_str("0.001").unwrap();
        assert_eq!(0.001, f64::from(significance));
        for invalid in ["0", "1", "0.0", "1.0", "-1", "2"] {
            let err = serde_json::from_str::<Significance>(invalid)
                .unwrap_err()
                .to_string();
            assert!(err.contains("`significance`"), "{invalid}: {err}");
        }
    }

    #[test]
    fn significance_from_str() {
        assert_eq!(0.05, f64::from("0.05".parse::<Significance>().unwrap()));
        "0".parse::<Significance>().unwrap_err();
        "one".parse::<Significance>().unwrap_err();
    }
}
