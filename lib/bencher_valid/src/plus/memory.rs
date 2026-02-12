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
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
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
                reason = "memory in bytes fits in i64 (max ~9.2 EB)"
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
    memory > 0
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::is_valid_memory;

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_memory(1));
        assert_eq!(true, is_valid_memory(0x0001_0000_0000)); // 4 GB
        assert_eq!(true, is_valid_memory(u64::MAX));

        assert_eq!(false, is_valid_memory(0));
    }
}
