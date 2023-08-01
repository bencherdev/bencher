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

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SampleSize(u32);

impl TryFrom<u32> for SampleSize {
    type Error = ValidError;

    fn try_from(sample_size: u32) -> Result<Self, Self::Error> {
        is_valid_sample_size(sample_size)
            .then_some(Self(sample_size))
            .ok_or(ValidError::SampleSize(sample_size))
    }
}

impl From<SampleSize> for u32 {
    fn from(sample_size: SampleSize) -> Self {
        sample_size.0
    }
}

impl SampleSize {
    pub const MIN: Self = Self(2);
    pub const THIRTY: Self = Self(30);
    pub const MAX: Self = Self(u32::MAX);
}

impl<'de> Deserialize<'de> for SampleSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(SampleSizeVisitor)
    }
}

struct SampleSizeVisitor;

impl<'de> Visitor<'de> for SampleSizeVisitor {
    type Value = SampleSize;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a statistical sample size greater than or equal to 2")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u32::try_from(value)
            .map_err(E::custom)?
            .try_into()
            .map_err(E::custom)
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_sample_size(sample_size: u32) -> bool {
    sample_size >= 2
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_sample_size, SampleSize};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_sample_size(SampleSize::MIN.into()));
        assert_eq!(true, is_valid_sample_size(2));
        assert_eq!(true, is_valid_sample_size(3));
        assert_eq!(true, is_valid_sample_size(4));
        assert_eq!(true, is_valid_sample_size(5));
        assert_eq!(true, is_valid_sample_size(SampleSize::THIRTY.into()));
        assert_eq!(true, is_valid_sample_size(SampleSize::MAX.into()));

        assert_eq!(false, is_valid_sample_size(0));
        assert_eq!(false, is_valid_sample_size(1));
    }
}
