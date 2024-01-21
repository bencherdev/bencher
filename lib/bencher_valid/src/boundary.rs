use derive_more::Display;
use ordered_float::OrderedFloat;
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

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Double))]
pub struct Boundary(OrderedFloat<f64>);

impl TryFrom<f64> for Boundary {
    type Error = ValidError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        Self::is_valid(boundary)
            .then(|| Self(boundary.into()))
            .ok_or(ValidError::Boundary(boundary))
    }
}

impl From<Boundary> for f64 {
    fn from(boundary: Boundary) -> Self {
        boundary.0.into()
    }
}

impl FromStr for Boundary {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(f64::from_str(s).map_err(ValidError::BoundaryStr)?)
    }
}

impl<'de> Deserialize<'de> for Boundary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_f64(BoundaryVisitor)
    }
}

struct BoundaryVisitor;

impl Visitor<'_> for BoundaryVisitor {
    type Value = Boundary;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a floating point boundary")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        #[allow(clippy::cast_precision_loss)]
        (value as f64).try_into().map_err(E::custom)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

impl Boundary {
    pub const ZERO: Self = Self(OrderedFloat(0.0));
    pub const MIN_STATISTICAL: Self = Self::FIFTY;
    pub const FIFTY: Self = Self(OrderedFloat(0.5));
    pub const FIFTY_FIVE: Self = Self(OrderedFloat(0.55));
    pub const SIXTY: Self = Self(OrderedFloat(0.6));
    pub const SIXTY_FIVE: Self = Self(OrderedFloat(0.65));
    pub const SEVENTY: Self = Self(OrderedFloat(0.7));
    pub const SEVENTY_FIVE: Self = Self(OrderedFloat(0.75));
    pub const EIGHTY: Self = Self(OrderedFloat(0.8));
    pub const EIGHTY_FIVE: Self = Self(OrderedFloat(0.85));
    pub const NINETY: Self = Self(OrderedFloat(0.9));
    pub const NINETY_FIVE: Self = Self(OrderedFloat(0.95));
    pub const NINETY_EIGHT: Self = Self(OrderedFloat(0.98));
    pub const NINETY_NINE: Self = Self(OrderedFloat(0.99));
    pub const THREE_NINES: Self = Self(OrderedFloat(0.999));
    pub const FOUR_NINES: Self = Self(OrderedFloat(0.9999));
    pub const FIVE_NINES: Self = Self(OrderedFloat(0.99999));
    #[allow(clippy::unreadable_literal)]
    pub const SIXTEEN_NINES: Self = Self(OrderedFloat(0.9999999999999999));
    pub const MAX_STATISTICAL: Self = Self::SIXTEEN_NINES;

    pub fn is_valid(boundary: f64) -> bool {
        is_valid_boundary(boundary)
    }

    pub fn is_valid_percentage(boundary: f64) -> bool {
        is_valid_percentage_boundary(boundary)
    }

    pub fn is_valid_normal(boundary: f64) -> bool {
        is_valid_normal_boundary(boundary)
    }

    pub fn is_valid_iqr(boundary: f64) -> bool {
        is_valid_iqr_boundary(boundary)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::Boundary;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Double, DB> for Boundary
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

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Double, DB> for Boundary
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
pub fn is_valid_boundary(boundary: f64) -> bool {
    boundary.is_finite()
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_percentage_boundary(boundary: f64) -> bool {
    boundary >= 0.0
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_normal_boundary(boundary: f64) -> bool {
    if boundary < 0.5 {
        false
    } else {
        boundary < 1.0
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_iqr_boundary(boundary: f64) -> bool {
    boundary >= 0.0
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_boundary, Boundary};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_boundary(0.0));
        assert_eq!(true, is_valid_boundary(-1.0));
        assert_eq!(true, is_valid_boundary(1.0));
        assert_eq!(true, is_valid_boundary(f64::MIN));
        assert_eq!(true, is_valid_boundary(f64::MAX));

        assert_eq!(false, is_valid_boundary(f64::INFINITY));
        assert_eq!(false, is_valid_boundary(f64::NEG_INFINITY));
        assert_eq!(false, is_valid_boundary(f64::NAN));
    }

    #[test]
    fn test_boundary_serde() {
        let boundary: Boundary = serde_json::from_str("0.0").unwrap();
        assert_eq!(Boundary(0.0.into()), boundary);
        let boundary: Boundary = serde_json::from_str("-1.0").unwrap();
        assert_eq!(Boundary((-1.0).into()), boundary);
        let boundary: Boundary = serde_json::from_str("1.0").unwrap();
        assert_eq!(Boundary(1.0.into()), boundary);

        let boundary = serde_json::from_str::<Boundary>(&f64::INFINITY.to_string());
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>(&f64::NEG_INFINITY.to_string());
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>(&f64::NAN.to_string());
        assert!(boundary.is_err());
    }
}
