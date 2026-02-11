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
pub struct Memory(u64);

impl TryFrom<u64> for Memory {
    type Error = ValidError;

    fn try_from(memory: u64) -> Result<Self, Self::Error> {
        is_valid_memory(memory)
            .then_some(Self(memory))
            .ok_or(ValidError::Memory(memory))
    }
}

impl From<Memory> for u64 {
    fn from(memory: Memory) -> Self {
        memory.0
    }
}

impl<'de> Deserialize<'de> for Memory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u64(MemoryVisitor)
    }
}

struct MemoryVisitor;

impl Visitor<'_> for MemoryVisitor {
    type Value = Memory;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a nonzero memory size in bytes")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

pub fn is_valid_memory(memory: u64) -> bool {
    memory > 0
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::is_valid_memory;

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_memory(1));
        assert_eq!(true, is_valid_memory(4_294_967_296)); // 4 GB
        assert_eq!(true, is_valid_memory(u64::MAX));

        assert_eq!(false, is_valid_memory(0));
    }
}
