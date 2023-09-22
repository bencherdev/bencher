use derive_more::Display;
use ordered_float::OrderedFloat;
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

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Boundary(OrderedFloat<f64>);

impl TryFrom<f64> for Boundary {
    type Error = ValidError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        is_valid_boundary(boundary)
            .then(|| Self(boundary.into()))
            .ok_or(ValidError::Boundary(boundary))
    }
}

impl From<Boundary> for f64 {
    fn from(boundary: Boundary) -> Self {
        boundary.0.into()
    }
}

impl Boundary {
    pub const MIN: Self = Self::FIFTY;
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
    pub const MAX: Self = Self::SIXTEEN_NINES;
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

impl<'de> Visitor<'de> for BoundaryVisitor {
    type Value = Boundary;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a statistical boundary between [0.5, 1.0)")
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_boundary(boundary: f64) -> bool {
    // The boundary must be greater than or equal to 0.5 and less than 1.0
    if boundary < 0.5 {
        false
    } else {
        boundary < 1.0
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_boundary, Boundary};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_boundary(0.499_999_999_999_999_99));
        assert_eq!(true, is_valid_boundary(Boundary::MIN.into()));
        assert_eq!(true, is_valid_boundary(Boundary::SIXTY.into()));
        assert_eq!(true, is_valid_boundary(Boundary::SEVENTY.into()));
        assert_eq!(true, is_valid_boundary(Boundary::EIGHTY.into()));
        assert_eq!(true, is_valid_boundary(Boundary::NINETY.into()));
        assert_eq!(true, is_valid_boundary(Boundary::MAX.into()));

        assert_eq!(false, is_valid_boundary(-1.0));
        assert_eq!(false, is_valid_boundary(0.0));
        assert_eq!(false, is_valid_boundary(0.1));
        assert_eq!(false, is_valid_boundary(0.2));
        assert_eq!(false, is_valid_boundary(0.3));
        assert_eq!(false, is_valid_boundary(0.4));
        assert_eq!(false, is_valid_boundary(0.499_999_999_999_999_9));
        assert_eq!(false, is_valid_boundary(0.999_999_999_999_999_99));
        assert_eq!(false, is_valid_boundary(1.0));
        assert_eq!(false, is_valid_boundary(2.0));
        assert_eq!(false, is_valid_boundary(3.0));
    }

    #[test]
    fn test_boundary_serde() {
        let boundary: Boundary = serde_json::from_str("0.49999999999999999").unwrap();
        assert_eq!(Boundary(0.5.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.5").unwrap();
        assert_eq!(Boundary(0.5.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.6").unwrap();
        assert_eq!(Boundary(0.6.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.7").unwrap();
        assert_eq!(Boundary(0.7.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.8").unwrap();
        assert_eq!(Boundary(0.8.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.9").unwrap();
        assert_eq!(Boundary(0.9.into()), boundary);
        let boundary: Boundary = serde_json::from_str("0.999999999999999").unwrap();
        assert_eq!(Boundary(0.999_999_999_999_999.into()), boundary);

        let boundary = serde_json::from_str::<Boundary>("-1.0");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.0");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.1");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.2");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.3");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.4");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.4999999999999999");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("0.99999999999999999");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("1.0");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("2.0");
        assert!(boundary.is_err());
        let boundary = serde_json::from_str::<Boundary>("3.0");
        assert!(boundary.is_err());
    }
}
