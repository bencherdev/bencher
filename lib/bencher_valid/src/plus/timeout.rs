use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const MIN_TIMEOUT: u32 = 1;
const MAX_TIMEOUT: u32 = i32::MAX as u32;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct Timeout(u32);

impl TryFrom<u32> for Timeout {
    type Error = ValidError;

    fn try_from(timeout: u32) -> Result<Self, Self::Error> {
        is_valid_timeout(timeout)
            .then_some(Self(timeout))
            .ok_or(ValidError::Timeout(timeout))
    }
}

impl From<Timeout> for u32 {
    fn from(timeout: Timeout) -> Self {
        timeout.0
    }
}

impl Timeout {
    pub const MIN: Self = Self(MIN_TIMEOUT);
    pub const MAX: Self = Self(MAX_TIMEOUT);
}

impl<'de> Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(TimeoutVisitor)
    }
}

struct TimeoutVisitor;

impl Visitor<'_> for TimeoutVisitor {
    type Value = Timeout;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a timeout in seconds between 1 and 2147483647")
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
    use super::Timeout;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Timeout
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
                reason = "Timeout max i32::MAX, always fits in i32"
            )]
            let val = self.0 as i32;
            out.set_value(val);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Timeout
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            #[expect(
                clippy::cast_sign_loss,
                reason = "Timeout stored as i32 but CHECK constraint ensures > 0"
            )]
            let timeout = i32::from_sql(bytes)? as u32;
            timeout.try_into().map_err(Into::into)
        }
    }
}

pub fn is_valid_timeout(timeout: u32) -> bool {
    (MIN_TIMEOUT..=MAX_TIMEOUT).contains(&timeout)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Timeout, is_valid_timeout};

    #[test]
    fn boundary() {
        assert_eq!(true, is_valid_timeout(Timeout::MIN.into()));
        assert_eq!(true, is_valid_timeout(1));
        assert_eq!(true, is_valid_timeout(3600));
        assert_eq!(true, is_valid_timeout(Timeout::MAX.into()));

        assert_eq!(false, is_valid_timeout(0));
        assert_eq!(false, is_valid_timeout(i32::MAX as u32 + 1));
    }
}
