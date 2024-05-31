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

const MAX_INDEX: u8 = 64;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Index(u8);

impl TryFrom<u8> for Index {
    type Error = ValidError;

    fn try_from(index: u8) -> Result<Self, Self::Error> {
        is_valid_index(index)
            .then_some(Self(index))
            .ok_or(ValidError::Index(index))
    }
}

impl From<Index> for u8 {
    fn from(index: Index) -> Self {
        index.0
    }
}

impl Index {
    pub const MIN: Self = Self(u8::MIN);
    pub const MAX: Self = Self(MAX_INDEX);
}

impl FromStr for Index {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(u8::from_str(s).map_err(ValidError::IndexStr)?)
    }
}

impl<'de> Deserialize<'de> for Index {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u8(IndexVisitor)
    }
}

struct IndexVisitor;

impl Visitor<'_> for IndexVisitor {
    type Value = Index;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a plot index greater than or equal to 0 and less than or equal to 64")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u8(u8::try_from(value).map_err(E::custom)?)
    }

    fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_index(index: u8) -> bool {
    index <= MAX_INDEX
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_index, Index};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_index(Index::MIN.into()));
        assert_eq!(true, is_valid_index(1));
        assert_eq!(true, is_valid_index(2));
        assert_eq!(true, is_valid_index(3));
        assert_eq!(true, is_valid_index(Index::MAX.into()));

        assert_eq!(false, is_valid_index(65));
    }
}
