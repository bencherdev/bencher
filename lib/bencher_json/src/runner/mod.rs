use bencher_valid::{DateTime, ResourceName, Slug};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod job;

pub use job::{JobStatus, JobUuid, JsonJob, JsonUpdateJob, JsonUpdateJobResponse};

crate::typed_uuid::typed_uuid!(RunnerUuid);

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
    Offline = 0,
    Idle = 1,
    Running = 2,
}

#[cfg(feature = "db")]
mod runner_state_db {
    use super::RunnerState;

    const OFFLINE_INT: i32 = 0;
    const IDLE_INT: i32 = 1;
    const RUNNING_INT: i32 = 2;

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

/// A benchmark runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunner {
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub state: RunnerState,
    pub locked: Option<DateTime>,
    pub archived: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}
