use std::{fmt, str::FromStr};

use chrono::{TimeZone, Utc};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct DateTime(chrono::DateTime<Utc>);

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct DateTimeMillis(TimestampMillis);

// Do not typeshare this type in order to obfuscate the i64
// https://github.com/1Password/typeshare/issues/24
#[derive(Debug, Display, Clone, Copy, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct TimestampMillis(i64);

impl TryFrom<i64> for DateTime {
    type Error = ValidError;

    fn try_from(timestamp: i64) -> Result<Self, Self::Error> {
        Utc.timestamp_opt(timestamp, 0)
            .single()
            .map(Self)
            .ok_or(ValidError::DateTime(timestamp))
    }
}

impl TryFrom<i64> for DateTimeMillis {
    type Error = ValidError;

    fn try_from(timestamp: i64) -> Result<Self, Self::Error> {
        date_time_millis(timestamp)
            .map(|dt| Self(TimestampMillis(dt.timestamp_millis())))
            .ok_or(ValidError::DateTimeMillis(timestamp))
    }
}

impl From<DateTimeMillis> for i64 {
    fn from(date_time: DateTimeMillis) -> Self {
        date_time.0 .0
    }
}

impl From<DateTimeMillis> for DateTime {
    fn from(date_time: DateTimeMillis) -> Self {
        let date_time = date_time_millis(date_time.into());
        debug_assert!(date_time.is_some(), "DateTimeMillis is invalid");
        Self(date_time.unwrap_or_default())
    }
}

impl From<DateTime> for DateTimeMillis {
    fn from(date_time: DateTime) -> Self {
        Self(TimestampMillis(date_time.0.timestamp_millis()))
    }
}

impl From<chrono::DateTime<Utc>> for DateTime {
    fn from(date_time: chrono::DateTime<Utc>) -> Self {
        Self(date_time)
    }
}

fn date_time_millis(millis: i64) -> Option<chrono::DateTime<Utc>> {
    const MILLIS_PER_SECOND: i64 = 1_000;
    const MILLIS_PER_NANO: i64 = 1_000_000;
    Utc.timestamp_opt(
        millis.checked_div(MILLIS_PER_SECOND)?,
        u32::try_from(
            millis
                .checked_rem(MILLIS_PER_SECOND)?
                .checked_mul(MILLIS_PER_NANO)?,
        )
        .ok()?,
    )
    .single()
}

impl DateTime {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    pub fn into_inner(self) -> chrono::DateTime<Utc> {
        self.0
    }
}

impl FromStr for DateTime {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(i64::from_str(s).map_err(ValidError::DateTimeStr)?)
    }
}

impl<'de> Deserialize<'de> for DateTimeMillis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_i64(DateTimeMillisVisitor)
    }
}

struct DateTimeMillisVisitor;

impl Visitor<'_> for DateTimeMillisVisitor {
    type Value = DateTimeMillis;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a date time timestamp in milliseconds")
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::DateTime;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for DateTime
    where
        DB: diesel::backend::Backend,
        for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(self.0.timestamp());
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for DateTime
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            i64::from_sql(bytes)?.try_into().map_err(Into::into)
        }
    }
}
