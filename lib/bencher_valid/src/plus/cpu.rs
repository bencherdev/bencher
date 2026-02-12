use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_CPU: u32 = 1;
const MAX_CPU: u32 = 256;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct Cpu(u32);

impl TryFrom<u32> for Cpu {
    type Error = ValidError;

    fn try_from(cpu: u32) -> Result<Self, Self::Error> {
        is_valid_cpu(cpu)
            .then_some(Self(cpu))
            .ok_or(ValidError::Cpu(cpu))
    }
}

impl From<Cpu> for u32 {
    fn from(cpu: Cpu) -> Self {
        cpu.0
    }
}

impl Cpu {
    pub const MIN: Self = Self(MIN_CPU);
    pub const MAX: Self = Self(MAX_CPU);
}

impl<'de> Deserialize<'de> for Cpu {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(CpuVisitor)
    }
}

struct CpuVisitor;

impl Visitor<'_> for CpuVisitor {
    type Value = Cpu;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a CPU count between 1 and 256")
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

#[cfg(feature = "db")]
mod db {
    use super::Cpu;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Cpu
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "CPU count max 256, always fits in i32"
            )]
            let val = self.0 as i32;
            out.set_value(val);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Cpu
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            #[expect(
                clippy::cast_sign_loss,
                reason = "CPU stored as i32 but CHECK constraint ensures > 0"
            )]
            let cpu = i32::from_sql(bytes)? as u32;
            cpu.try_into().map_err(Into::into)
        }
    }
}

pub fn is_valid_cpu(cpu: u32) -> bool {
    (MIN_CPU..=MAX_CPU).contains(&cpu)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Cpu, is_valid_cpu};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_cpu(Cpu::MIN.into()));
        assert_eq!(true, is_valid_cpu(1));
        assert_eq!(true, is_valid_cpu(2));
        assert_eq!(true, is_valid_cpu(128));
        assert_eq!(true, is_valid_cpu(Cpu::MAX.into()));

        assert_eq!(false, is_valid_cpu(0));
        assert_eq!(false, is_valid_cpu(257));
    }
}
