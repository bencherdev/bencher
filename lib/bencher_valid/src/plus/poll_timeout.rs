use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_POLL_TIMEOUT: u32 = 1;
const MAX_POLL_TIMEOUT: u32 = 600;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PollTimeout(u32);

impl TryFrom<u32> for PollTimeout {
    type Error = ValidError;

    fn try_from(poll_timeout: u32) -> Result<Self, Self::Error> {
        is_valid_poll_timeout(poll_timeout)
            .then_some(Self(poll_timeout))
            .ok_or(ValidError::PollTimeout(poll_timeout))
    }
}

impl From<PollTimeout> for u32 {
    fn from(poll_timeout: PollTimeout) -> Self {
        poll_timeout.0
    }
}

impl PollTimeout {
    pub const MIN: Self = Self(MIN_POLL_TIMEOUT);
    pub const MAX: Self = Self(MAX_POLL_TIMEOUT);
}

impl<'de> Deserialize<'de> for PollTimeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(PollTimeoutVisitor)
    }
}

struct PollTimeoutVisitor;

impl Visitor<'_> for PollTimeoutVisitor {
    type Value = PollTimeout;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a poll timeout in seconds between 1 and 600")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u32(u32::try_from(v).map_err(E::custom)?)
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

pub fn is_valid_poll_timeout(poll_timeout: u32) -> bool {
    (MIN_POLL_TIMEOUT..=MAX_POLL_TIMEOUT).contains(&poll_timeout)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{PollTimeout, is_valid_poll_timeout};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_poll_timeout(PollTimeout::MIN.into()));
        assert_eq!(true, is_valid_poll_timeout(1));
        assert_eq!(true, is_valid_poll_timeout(300));
        assert_eq!(true, is_valid_poll_timeout(PollTimeout::MAX.into()));

        assert_eq!(false, is_valid_poll_timeout(0));
        assert_eq!(false, is_valid_poll_timeout(601));
    }
}
