use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::JsonUser;

crate::typed_uuid::typed_uuid!(SessionUuid);

/// The signed-in reader's browser session.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSession {
    pub user: JsonUser,
    pub method: SessionMethod,
    pub created: DateTime,
    pub expiration: DateTime,
}

/// How the reader signed in to a session.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum SessionMethod {
    Email = EMAIL_INT,
    Github = GITHUB_INT,
    Google = GOOGLE_INT,
    Passkey = PASSKEY_INT,
}

const EMAIL_INT: i32 = 0;
const GITHUB_INT: i32 = 1;
const GOOGLE_INT: i32 = 2;
const PASSKEY_INT: i32 = 3;

#[cfg(feature = "db")]
mod session_method {
    use super::{EMAIL_INT, GITHUB_INT, GOOGLE_INT, PASSKEY_INT, SessionMethod};

    #[derive(Debug, thiserror::Error)]
    pub enum SessionMethodError {
        #[error("Invalid session method value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for SessionMethod
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Email => EMAIL_INT.to_sql(out),
                Self::Github => GITHUB_INT.to_sql(out),
                Self::Google => GOOGLE_INT.to_sql(out),
                Self::Passkey => PASSKEY_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for SessionMethod
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                EMAIL_INT => Ok(Self::Email),
                GITHUB_INT => Ok(Self::Github),
                GOOGLE_INT => Ok(Self::Google),
                PASSKEY_INT => Ok(Self::Passkey),
                value => Err(Box::new(SessionMethodError::Invalid(value))),
            }
        }
    }
}
