#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const OFFLINE_INT: i32 = 0;
const IDLE_INT: i32 = 1;
const RUNNING_INT: i32 = 2;

/// Runner state
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum RunnerState {
    #[default]
    Offline = OFFLINE_INT,
    Idle = IDLE_INT,
    Running = RUNNING_INT,
}

#[cfg(feature = "db")]
mod runner_state_db {
    use super::{IDLE_INT, OFFLINE_INT, RUNNING_INT, RunnerState};

    #[derive(Debug, thiserror::Error)]
    pub enum RunnerStateError {
        #[error("Invalid runner state value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for RunnerState
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Offline => OFFLINE_INT.to_sql(out),
                Self::Idle => IDLE_INT.to_sql(out),
                Self::Running => RUNNING_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for RunnerState
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                OFFLINE_INT => Ok(Self::Offline),
                IDLE_INT => Ok(Self::Idle),
                RUNNING_INT => Ok(Self::Running),
                value => Err(Box::new(RunnerStateError::Invalid(value))),
            }
        }
    }
}
