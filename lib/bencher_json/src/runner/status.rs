use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{health::JsonRunnerHealth, websocket::PauseReason};

const READY_INT: i32 = 0;
const PAUSED_INT: i32 = 1;

/// A runner's status, as it last changed
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunnerStatus {
    /// Whether the runner takes jobs
    pub availability: RunnerAvailability,
    /// Why the runner is paused, empty when it is ready
    #[typeshare(typescript(type = "Record<string, unknown>[]"))]
    pub reasons: Vec<PauseReason>,
    /// When the pause began, by the runner's clock
    pub since: Option<DateTime>,
    /// The runner host's disk health, absent until the runner reports it
    #[typeshare(typescript(type = "Record<string, unknown> | undefined"))]
    pub health: Option<JsonRunnerHealth>,
    /// When the server stored this status, which it does only when the status changes
    pub changed: DateTime,
}

/// Whether a runner takes jobs
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum RunnerAvailability {
    /// The runner takes jobs
    Ready = READY_INT,
    /// The runner takes no job until every pause reason clears
    Paused = PAUSED_INT,
}

#[cfg(feature = "db")]
mod runner_availability_db {
    use super::{PAUSED_INT, READY_INT, RunnerAvailability};

    #[derive(Debug, thiserror::Error)]
    pub enum RunnerAvailabilityError {
        #[error("Invalid runner availability value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for RunnerAvailability
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Ready => READY_INT.to_sql(out),
                Self::Paused => PAUSED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for RunnerAvailability
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                READY_INT => Ok(Self::Ready),
                PAUSED_INT => Ok(Self::Paused),
                value => Err(Box::new(RunnerAvailabilityError::Invalid(value))),
            }
        }
    }
}
