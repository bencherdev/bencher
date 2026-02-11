use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_GRACE_PERIOD: u64 = 10;
const MAX_GRACE_PERIOD: u64 = 600;

#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct GracePeriod(u64);

impl TryFrom<u64> for GracePeriod {
    type Error = ValidError;

    fn try_from(period: u64) -> Result<Self, Self::Error> {
        is_valid_grace_period(period)
            .then_some(Self(period))
            .ok_or(ValidError::GracePeriod(period))
    }
}

impl From<GracePeriod> for u64 {
    fn from(period: GracePeriod) -> Self {
        period.0
    }
}

impl GracePeriod {
    pub const MIN: Self = Self(MIN_GRACE_PERIOD);
    pub const MAX: Self = Self(MAX_GRACE_PERIOD);
}

impl<'de> Deserialize<'de> for GracePeriod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u64(GracePeriodVisitor)
    }
}

struct GracePeriodVisitor;

impl Visitor<'_> for GracePeriodVisitor {
    type Value = GracePeriod;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a grace period in seconds between 10 and 600")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

pub fn is_valid_grace_period(period: u64) -> bool {
    (MIN_GRACE_PERIOD..=MAX_GRACE_PERIOD).contains(&period)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{GracePeriod, is_valid_grace_period};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_grace_period(GracePeriod::MIN.into()));
        assert_eq!(true, is_valid_grace_period(10));
        assert_eq!(true, is_valid_grace_period(60));
        assert_eq!(true, is_valid_grace_period(GracePeriod::MAX.into()));

        assert_eq!(false, is_valid_grace_period(0));
        assert_eq!(false, is_valid_grace_period(9));
        assert_eq!(false, is_valid_grace_period(601));
    }
}
