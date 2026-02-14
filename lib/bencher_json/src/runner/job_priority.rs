#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const UNCLAIMED_PRIORITY_INT: i32 = 0;
const FREE_PRIORITY_INT: i32 = 100;
const TEAM_PRIORITY_INT: i32 = 200;
const ENTERPRISE_PRIORITY_INT: i32 = 300;

/// Job priority — determines scheduling order and concurrency limits.
///
/// Priority tiers:
/// - Enterprise (300): Unlimited concurrent jobs
/// - Team (200): Unlimited concurrent jobs
/// - Free (100): 1 concurrent job per organization
/// - Unclaimed (0): 1 concurrent job per source IP
#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobPriority {
    #[default]
    Unclaimed = UNCLAIMED_PRIORITY_INT,
    Free = FREE_PRIORITY_INT,
    Team = TEAM_PRIORITY_INT,
    Enterprise = ENTERPRISE_PRIORITY_INT,
}

impl JobPriority {
    /// Returns true if this priority tier has unlimited concurrency.
    pub fn is_unlimited(&self) -> bool {
        matches!(self, Self::Team | Self::Enterprise)
    }

    /// Returns true if this priority is in the Free tier.
    pub fn is_free(&self) -> bool {
        matches!(self, Self::Free)
    }
}

impl From<JobPriority> for i32 {
    fn from(priority: JobPriority) -> Self {
        priority as Self
    }
}

#[cfg(feature = "db")]
mod job_priority_db {
    use super::{
        ENTERPRISE_PRIORITY_INT, FREE_PRIORITY_INT, JobPriority, TEAM_PRIORITY_INT,
        UNCLAIMED_PRIORITY_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum JobPriorityError {
        #[error("Invalid job priority value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobPriority
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Unclaimed => UNCLAIMED_PRIORITY_INT.to_sql(out),
                Self::Free => FREE_PRIORITY_INT.to_sql(out),
                Self::Team => TEAM_PRIORITY_INT.to_sql(out),
                Self::Enterprise => ENTERPRISE_PRIORITY_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobPriority
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                UNCLAIMED_PRIORITY_INT => Ok(Self::Unclaimed),
                FREE_PRIORITY_INT => Ok(Self::Free),
                TEAM_PRIORITY_INT => Ok(Self::Team),
                ENTERPRISE_PRIORITY_INT => Ok(Self::Enterprise),
                value => Err(Box::new(JobPriorityError::Invalid(value))),
            }
        }
    }
}
