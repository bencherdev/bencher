use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_MEMORY: u64 = 1;
const MAX_MEMORY: u64 = i64::MAX as u64;

#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct Memory(u64);

impl Memory {
    pub const MIN: Self = Self(MIN_MEMORY);
    pub const MAX: Self = Self(MAX_MEMORY);

    /// Convert bytes to mebibytes (MiB), rounding up.
    ///
    /// Returns `u64` since consumers like `create_ext4_with_size` expect `u64`.
    /// For Firecracker's `u32` fields, callers cast at the boundary.
    #[must_use]
    pub const fn to_mib(self) -> u64 {
        let bytes = self.0;
        if bytes == 0 {
            return 0;
        }
        bytes.div_ceil(1024 * 1024)
    }

    /// Create from a value in mebibytes (MiB).
    #[must_use]
    pub const fn from_mib(mib: u64) -> Self {
        Self(mib.saturating_mul(1024 * 1024))
    }
}

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

#[cfg(feature = "db")]
mod db {
    use super::Memory;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Memory
    where
        DB: diesel::backend::Backend,
        for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "validated max is i64::MAX, cast is safe"
            )]
            let val = self.0 as i64;
            out.set_value(val);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Memory
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            #[expect(
                clippy::cast_sign_loss,
                reason = "memory stored as i64 but CHECK constraint ensures > 0"
            )]
            let memory = i64::from_sql(bytes)? as u64;
            memory.try_into().map_err(Into::into)
        }
    }
}

pub fn is_valid_memory(memory: u64) -> bool {
    (MIN_MEMORY..=MAX_MEMORY).contains(&memory)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Memory, is_valid_memory};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_memory(Memory::MIN.into()));
        assert_eq!(true, is_valid_memory(1));
        assert_eq!(true, is_valid_memory(0x0001_0000_0000)); // 4 GB
        assert_eq!(true, is_valid_memory(Memory::MAX.into()));

        assert_eq!(false, is_valid_memory(0));
        assert_eq!(false, is_valid_memory(u64::MAX));
    }

    #[test]
    fn to_mib_exact() {
        let m = Memory::try_from(512 * 1024 * 1024u64).unwrap();
        assert_eq!(m.to_mib(), 512);
    }

    #[test]
    fn to_mib_rounds_up() {
        let m = Memory::try_from(512 * 1024 * 1024u64 + 1).unwrap();
        assert_eq!(m.to_mib(), 513);
    }

    #[test]
    fn to_mib_one_byte() {
        let m = Memory::try_from(1u64).unwrap();
        assert_eq!(m.to_mib(), 1); // rounds up
    }

    #[test]
    fn from_mib_round_trip() {
        let m = Memory::from_mib(2048);
        assert_eq!(m.to_mib(), 2048);
        assert_eq!(u64::from(m), 2048 * 1024 * 1024);
    }

    #[test]
    fn from_mib_overflow_saturates() {
        let m = Memory::from_mib(u64::MAX);
        assert_eq!(u64::from(m), u64::MAX);
    }

    #[test]
    fn from_mib_zero() {
        let m = Memory::from_mib(0);
        assert_eq!(u64::from(m), 0);
    }
}
