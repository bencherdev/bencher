use std::{fmt, str::FromStr};

use chrono::{TimeZone as _, Utc};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
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
        date_time.0.0
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
    /// Fixed deterministic [`DateTime`] for tests: 06:23 UTC, 11 July 2024.
    #[cfg(any(test, feature = "test-clock"))]
    pub const TEST: Self = Self(match chrono::DateTime::from_timestamp(1_720_678_980, 0) {
        Some(dt) => dt,
        None => panic!("invalid test timestamp"),
    });

    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    pub fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }

    /// Compute wall-clock duration in fractional seconds from `self` to `now`.
    /// Uses millisecond precision and clamps to zero.
    #[expect(
        clippy::cast_precision_loss,
        reason = "millisecond precision is sufficient for elapsed time"
    )]
    pub fn elapsed_secs(self, now: Self) -> f64 {
        ((now.timestamp_millis() - self.timestamp_millis()) as f64 / 1000.0).max(0.0)
    }

    pub fn into_inner(self) -> chrono::DateTime<Utc> {
        self.0
    }
}

impl std::ops::Add<chrono::Duration> for DateTime {
    type Output = Self;

    fn add(self, rhs: chrono::Duration) -> Self {
        Self(self.0 + rhs)
    }
}

impl std::ops::Sub<chrono::Duration> for DateTime {
    type Output = Self;

    fn sub(self, rhs: chrono::Duration) -> Self {
        Self(self.0 - rhs)
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

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
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

#[cfg(test)]
mod tests {
    use super::{DateTime, DateTimeMillis};
    use crate::ValidError;
    use chrono::Duration;
    use pretty_assertions::assert_eq;

    /// The timestamp behind [`DateTime::TEST`]: 06:23:00 UTC, 11 July 2024.
    pub(super) const TEST_TIMESTAMP: i64 = 1_720_678_980;
    pub(super) const TEST_TIMESTAMP_MILLIS: i64 = 1_720_678_980_000;
    /// 9999-12-31T23:59:59Z, the largest commonly used four digit year timestamp.
    const YEAR_9999_TIMESTAMP: i64 = 253_402_300_799;
    const TEST_RFC3339_JSON: &str = "\"2024-07-11T06:23:00Z\"";

    #[test]
    fn elapsed_secs_positive_duration() {
        let start = DateTime::TEST;
        let now = start + Duration::seconds(5);
        assert!((start.elapsed_secs(now) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn elapsed_secs_zero_duration() {
        let dt = DateTime::TEST;
        assert!(dt.elapsed_secs(dt).abs() < f64::EPSILON);
    }

    #[test]
    fn elapsed_secs_negative_clamps_to_zero() {
        let start = DateTime::TEST + Duration::seconds(5);
        let now = DateTime::TEST;
        assert!(start.elapsed_secs(now).abs() < f64::EPSILON);
    }

    #[test]
    fn elapsed_secs_millisecond_precision() {
        let start = DateTime::TEST;
        let now = start + Duration::milliseconds(1500);
        assert!((start.elapsed_secs(now) - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn date_time_try_from_valid() {
        for timestamp in [0, 1, -1, TEST_TIMESTAMP, YEAR_9999_TIMESTAMP] {
            let date_time = DateTime::try_from(timestamp).expect("valid timestamp");
            assert_eq!(timestamp, date_time.timestamp(), "{timestamp}");
        }
        assert_eq!(
            DateTime::TEST,
            DateTime::try_from(TEST_TIMESTAMP).expect("test timestamp")
        );
    }

    #[test]
    fn date_time_try_from_invalid() {
        for timestamp in [i64::MAX, i64::MIN] {
            let error = DateTime::try_from(timestamp).expect_err("out of range timestamp");
            assert!(
                matches!(error, ValidError::DateTime(t) if t == timestamp),
                "{error}"
            );
        }
    }

    #[test]
    fn date_time_millis_try_from_valid() {
        for millis in [
            0,
            1,
            999,
            TEST_TIMESTAMP_MILLIS,
            TEST_TIMESTAMP_MILLIS + 123,
            -1_000,
        ] {
            let date_time_millis = DateTimeMillis::try_from(millis).expect("valid millis");
            assert_eq!(millis, i64::from(date_time_millis), "{millis}");
        }
    }

    #[test]
    fn date_time_millis_try_from_invalid() {
        // Negative timestamps with sub-second precision are rejected:
        // the nanosecond remainder is negative and fails the `u32` conversion.
        for millis in [-1, -999, -1_001, i64::MAX, i64::MIN] {
            let error = DateTimeMillis::try_from(millis).expect_err("invalid millis");
            assert!(
                matches!(error, ValidError::DateTimeMillis(m) if m == millis),
                "{error}"
            );
        }
    }

    #[test]
    fn date_time_millis_round_trip() {
        let date_time_millis = DateTimeMillis::from(DateTime::TEST);
        assert_eq!(TEST_TIMESTAMP_MILLIS, i64::from(date_time_millis));
        assert_eq!(DateTime::TEST, DateTime::from(date_time_millis));

        let sub_second_millis = TEST_TIMESTAMP_MILLIS + 123;
        let date_time_millis = DateTimeMillis::try_from(sub_second_millis).expect("valid millis");
        let date_time = DateTime::from(date_time_millis);
        assert_eq!(sub_second_millis, date_time.timestamp_millis());
        assert_eq!(TEST_TIMESTAMP, date_time.timestamp());
    }

    #[test]
    fn date_time_from_str_valid() {
        let date_time: DateTime = "1720678980".parse().expect("valid timestamp string");
        assert_eq!(DateTime::TEST, date_time);
    }

    #[test]
    fn date_time_from_str_invalid() {
        let error = "abc".parse::<DateTime>().expect_err("non-numeric string");
        assert!(matches!(error, ValidError::DateTimeStr(_)), "{error}");

        let error = "".parse::<DateTime>().expect_err("empty string");
        assert!(matches!(error, ValidError::DateTimeStr(_)), "{error}");

        let error = "9223372036854775807"
            .parse::<DateTime>()
            .expect_err("out of range timestamp string");
        assert!(
            matches!(error, ValidError::DateTime(t) if t == i64::MAX),
            "{error}"
        );
    }

    #[test]
    fn date_time_serde_round_trip() {
        let json = serde_json::to_string(&DateTime::TEST).expect("serialize date time");
        assert_eq!(TEST_RFC3339_JSON, json);
        let date_time: DateTime = serde_json::from_str(&json).expect("deserialize date time");
        assert_eq!(DateTime::TEST, date_time);
    }

    // Note: `DateTimeMillis` only implements `Deserialize`, not `Serialize`.
    // The visitor only implements `visit_i64`, which is what string-based
    // deserializers (e.g. query parameters) call.
    #[test]
    fn date_time_millis_deserialize_i64() {
        use serde::{
            Deserialize as _,
            de::value::{Error as DeserializeError, I64Deserializer},
        };

        let millis = TEST_TIMESTAMP_MILLIS + 123;
        let date_time_millis =
            DateTimeMillis::deserialize(I64Deserializer::<DeserializeError>::new(millis))
                .expect("deserialize millis");
        assert_eq!(millis, i64::from(date_time_millis));

        for invalid in [i64::MAX, i64::MIN, -1] {
            DateTimeMillis::deserialize(I64Deserializer::<DeserializeError>::new(invalid))
                .unwrap_err();
        }
    }

    #[test]
    fn date_time_millis_serde_json() {
        // serde_json parses positive integers as `u64` and calls `visit_u64`,
        // which the visitor does not implement, so positive JSON integers are
        // rejected even when they are valid millisecond timestamps.
        serde_json::from_str::<DateTimeMillis>("1720678980123").unwrap_err();
        // Negative integers are parsed as `i64` and hit `visit_i64`,
        // so a negative whole-second timestamp is accepted.
        let date_time_millis =
            serde_json::from_str::<DateTimeMillis>("-1000").expect("negative whole second");
        assert_eq!(-1_000, i64::from(date_time_millis));
    }

    #[test]
    fn date_time_millis_serde_invalid() {
        serde_json::from_str::<DateTimeMillis>("9223372036854775807").unwrap_err();
        serde_json::from_str::<DateTimeMillis>("-1").unwrap_err();
        serde_json::from_str::<DateTimeMillis>("\"1720678980123\"").unwrap_err();
    }
}

#[cfg(test)]
#[cfg(feature = "db")]
mod db_tests {
    use diesel::{Connection as _, IntoSql as _, RunQueryDsl as _, SqliteConnection};
    use pretty_assertions::assert_eq;

    use super::{
        DateTime, DateTimeMillis,
        tests::{TEST_TIMESTAMP, TEST_TIMESTAMP_MILLIS},
    };

    fn connection() -> SqliteConnection {
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database")
    }

    #[test]
    fn date_time_sql_round_trip() {
        let mut conn = connection();
        let date_time: DateTime =
            diesel::select(DateTime::TEST.into_sql::<diesel::sql_types::BigInt>())
                .get_result(&mut conn)
                .expect("Failed to round trip DateTime");
        assert_eq!(DateTime::TEST, date_time);

        let raw: i64 = diesel::select(DateTime::TEST.into_sql::<diesel::sql_types::BigInt>())
            .get_result(&mut conn)
            .expect("Failed to select DateTime as i64");
        assert_eq!(TEST_TIMESTAMP, raw);
    }

    #[test]
    fn date_time_to_sql_truncates_sub_second() {
        let mut conn = connection();
        let date_time_millis =
            DateTimeMillis::try_from(TEST_TIMESTAMP_MILLIS + 123).expect("valid millis");
        let date_time = DateTime::from(date_time_millis);
        let raw: i64 = diesel::select(date_time.into_sql::<diesel::sql_types::BigInt>())
            .get_result(&mut conn)
            .expect("Failed to select DateTime as i64");
        // `ToSql` stores whole seconds, so the milliseconds are dropped.
        assert_eq!(TEST_TIMESTAMP, raw);
    }

    #[test]
    fn date_time_from_sql_invalid() {
        let mut conn = connection();
        let error = diesel::select(i64::MAX.into_sql::<diesel::sql_types::BigInt>())
            .get_result::<DateTime>(&mut conn)
            .expect_err("Out of range i64 should not deserialize to DateTime");
        let message = error.to_string();
        assert!(
            message.contains(&format!("Failed to validate date time: {}", i64::MAX)),
            "{message}"
        );
    }
}
