use std::str::FromStr;

use crate::{DateTime, ValidError};

/// The most seconds an OAuth client allows since its user last signed in (OIDC Core, section 3.1.2.1).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct OAuthMaxAge(u32);

impl OAuthMaxAge {
    /// Whether a sign in at `auth_time` is still recent enough at `now`.
    pub fn admits(self, auth_time: DateTime, now: DateTime) -> bool {
        auth_time.timestamp().saturating_add(i64::from(self.0)) >= now.timestamp()
    }
}

impl FromStr for OAuthMaxAge {
    type Err = ValidError;

    fn from_str(max_age: &str) -> Result<Self, Self::Err> {
        max_age
            .parse()
            .map(Self)
            .map_err(|_err| ValidError::OAuthMaxAge)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::OAuthMaxAge;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for OAuthMaxAge
    where
        DB: diesel::backend::Backend,
        for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i64::from(self.0));
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for OAuthMaxAge
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(Self(u32::try_from(i64::from_sql(bytes)?)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::DateTime;

    use super::OAuthMaxAge;

    // Kills a sign in counted as recent one second past its max age, or not counted on the last one.
    #[test]
    fn admits_a_sign_in_until_its_max_age_passes() {
        let max_age: OAuthMaxAge = "60".parse().unwrap();
        let auth_time = DateTime::TEST;
        let at = |seconds: i64| DateTime::try_from(auth_time.timestamp() + seconds).unwrap();
        assert!(max_age.admits(auth_time, at(60)));
        assert!(!max_age.admits(auth_time, at(61)));
    }
}
