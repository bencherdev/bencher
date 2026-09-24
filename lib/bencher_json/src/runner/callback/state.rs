#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const PENDING_INT: i32 = 0;
const DELIVERED_INT: i32 = 2;
const FAILED_INT: i32 = 3;
const SKIPPED_INT: i32 = 4;

/// The delivery of a job's callback
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobCallback {
    pub state: JobCallbackState,
    /// The HTTP status of the last response, if any attempt got one
    pub status: Option<u16>,
}

/// Job callback state
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobCallbackState {
    Pending = PENDING_INT,
    Delivered = DELIVERED_INT,
    Failed = FAILED_INT,
    Skipped = SKIPPED_INT,
}

#[cfg(feature = "db")]
mod job_callback_state_db {
    use super::{DELIVERED_INT, FAILED_INT, JobCallbackState, PENDING_INT, SKIPPED_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum JobCallbackStateError {
        #[error("Invalid job callback state value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobCallbackState
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Pending => PENDING_INT.to_sql(out),
                Self::Delivered => DELIVERED_INT.to_sql(out),
                Self::Failed => FAILED_INT.to_sql(out),
                Self::Skipped => SKIPPED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobCallbackState
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                PENDING_INT => Ok(Self::Pending),
                DELIVERED_INT => Ok(Self::Delivered),
                FAILED_INT => Ok(Self::Failed),
                SKIPPED_INT => Ok(Self::Skipped),
                value => Err(Box::new(JobCallbackStateError::Invalid(value))),
            }
        }
    }
}
