use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_HEARTBEAT_TIMEOUT: u32 = 1;
const MAX_HEARTBEAT_TIMEOUT: u32 = 900;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct HeartbeatTimeout(u32);

impl TryFrom<u32> for HeartbeatTimeout {
    type Error = ValidError;

    fn try_from(timeout: u32) -> Result<Self, Self::Error> {
        is_valid_heartbeat_timeout(timeout)
            .then_some(Self(timeout))
            .ok_or(ValidError::HeartbeatTimeout(timeout))
    }
}

impl From<HeartbeatTimeout> for u32 {
    fn from(timeout: HeartbeatTimeout) -> Self {
        timeout.0
    }
}

impl HeartbeatTimeout {
    pub const MIN: Self = Self(MIN_HEARTBEAT_TIMEOUT);
    pub const MAX: Self = Self(MAX_HEARTBEAT_TIMEOUT);
}

impl<'de> Deserialize<'de> for HeartbeatTimeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(HeartbeatTimeoutVisitor)
    }
}

struct HeartbeatTimeoutVisitor;

impl Visitor<'_> for HeartbeatTimeoutVisitor {
    type Value = HeartbeatTimeout;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a heartbeat timeout in seconds between 1 and 900")
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

pub fn is_valid_heartbeat_timeout(timeout: u32) -> bool {
    (MIN_HEARTBEAT_TIMEOUT..=MAX_HEARTBEAT_TIMEOUT).contains(&timeout)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{HeartbeatTimeout, is_valid_heartbeat_timeout};

    #[test]
    fn boundary() {
        assert_eq!(
            true,
            is_valid_heartbeat_timeout(HeartbeatTimeout::MIN.into())
        );
        assert_eq!(true, is_valid_heartbeat_timeout(1));
        assert_eq!(true, is_valid_heartbeat_timeout(90));
        assert_eq!(
            true,
            is_valid_heartbeat_timeout(HeartbeatTimeout::MAX.into())
        );

        assert_eq!(false, is_valid_heartbeat_timeout(0));
        assert_eq!(false, is_valid_heartbeat_timeout(901));
    }
}
