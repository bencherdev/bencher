use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Disk(u64);

impl TryFrom<u64> for Disk {
    type Error = ValidError;

    fn try_from(disk: u64) -> Result<Self, Self::Error> {
        is_valid_disk(disk)
            .then_some(Self(disk))
            .ok_or(ValidError::Disk(disk))
    }
}

impl From<Disk> for u64 {
    fn from(disk: Disk) -> Self {
        disk.0
    }
}

impl<'de> Deserialize<'de> for Disk {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u64(DiskVisitor)
    }
}

struct DiskVisitor;

impl Visitor<'_> for DiskVisitor {
    type Value = Disk;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a nonzero disk size in bytes")
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u64(u64::from(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

pub fn is_valid_disk(disk: u64) -> bool {
    disk > 0
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::is_valid_disk;

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_disk(1));
        assert_eq!(true, is_valid_disk(10_737_418_240)); // 10 GB
        assert_eq!(true, is_valid_disk(u64::MAX));

        assert_eq!(false, is_valid_disk(0));
    }
}
