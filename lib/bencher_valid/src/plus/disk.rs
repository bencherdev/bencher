use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_DISK: u64 = 1;
const MAX_DISK: u64 = i64::MAX as u64;

#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct Disk(u64);

impl Disk {
    pub const MIN: Self = Self(MIN_DISK);
    pub const MAX: Self = Self(MAX_DISK);
}

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

#[cfg(feature = "db")]
mod db {
    use super::Disk;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Disk
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

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Disk
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            #[expect(
                clippy::cast_sign_loss,
                reason = "disk stored as i64 but CHECK constraint ensures > 0"
            )]
            let disk = i64::from_sql(bytes)? as u64;
            disk.try_into().map_err(Into::into)
        }
    }
}

pub fn is_valid_disk(disk: u64) -> bool {
    (MIN_DISK..=MAX_DISK).contains(&disk)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Disk, is_valid_disk};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_disk(Disk::MIN.into()));
        assert_eq!(true, is_valid_disk(1));
        assert_eq!(true, is_valid_disk(10_737_418_240)); // 10 GB
        assert_eq!(true, is_valid_disk(Disk::MAX.into()));

        assert_eq!(false, is_valid_disk(0));
        assert_eq!(false, is_valid_disk(u64::MAX));
    }
}
