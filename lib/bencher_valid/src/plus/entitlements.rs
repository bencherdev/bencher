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

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Entitlements(u32);

impl TryFrom<u32> for Entitlements {
    type Error = ValidError;

    fn try_from(entitlements: u32) -> Result<Self, Self::Error> {
        is_valid_entitlements(entitlements)
            .then_some(Self(entitlements))
            .ok_or(ValidError::Entitlements(entitlements))
    }
}

impl From<Entitlements> for u64 {
    fn from(entitlements: Entitlements) -> Self {
        u64::from(entitlements.0)
    }
}

impl From<Entitlements> for u32 {
    fn from(entitlements: Entitlements) -> Self {
        entitlements.0
    }
}

impl Entitlements {
    pub const MIN: Self = Self(1);
    pub const MAX: Self = Self(u32::MAX);
}

impl FromStr for Entitlements {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(u32::from_str(s).map_err(ValidError::EntitlementsStr)?)
    }
}

impl<'de> Deserialize<'de> for Entitlements {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(EntitlementsVisitor)
    }
}

struct EntitlementsVisitor;

impl Visitor<'_> for EntitlementsVisitor {
    type Value = Entitlements;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a statistical sample size greater than or equal to 2")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u32(u32::try_from(value).map_err(E::custom)?)
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

impl Default for Entitlements {
    fn default() -> Self {
        Self::MIN
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_entitlements(entitlements: u32) -> bool {
    entitlements > 0
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_entitlements, Entitlements};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_entitlements(Entitlements::default().into()));
        assert_eq!(true, is_valid_entitlements(Entitlements::MIN.into()));
        assert_eq!(true, is_valid_entitlements(2));
        assert_eq!(true, is_valid_entitlements(3));
        assert_eq!(true, is_valid_entitlements(4));
        assert_eq!(true, is_valid_entitlements(5));
        assert_eq!(true, is_valid_entitlements(Entitlements::MAX.into()));

        assert_eq!(false, is_valid_entitlements(0));
    }
}
