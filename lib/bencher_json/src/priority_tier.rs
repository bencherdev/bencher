use serde::{Deserialize, Serialize};

const UNCLAIMED_PRIORITY_INT: i32 = 0;
const FREE_PRIORITY_INT: i32 = 100;
const PLUS_PRIORITY_INT: i32 = 200;

/// Priority tier — determines scheduling order and concurrency limits.
///
/// Priority tiers:
/// - Plus (200): Unlimited concurrent jobs
/// - Free (100): 1 concurrent job per organization
/// - Unclaimed (0): 1 concurrent job per source IP
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum PriorityTier {
    #[default]
    Unclaimed = UNCLAIMED_PRIORITY_INT,
    Free = FREE_PRIORITY_INT,
    Plus = PLUS_PRIORITY_INT,
}

impl PriorityTier {
    /// Returns true if this priority tier has unlimited concurrency.
    pub fn is_unlimited(&self) -> bool {
        matches!(self, Self::Plus)
    }

    /// Returns true if this priority is in the Free tier.
    pub fn is_free(&self) -> bool {
        matches!(self, Self::Free)
    }
}

impl From<PriorityTier> for i32 {
    fn from(priority: PriorityTier) -> Self {
        priority as Self
    }
}

#[cfg(feature = "db")]
mod priority_tier_db {
    use super::{FREE_PRIORITY_INT, PLUS_PRIORITY_INT, PriorityTier, UNCLAIMED_PRIORITY_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum PriorityTierError {
        #[error("Invalid priority tier value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for PriorityTier
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
                Self::Plus => PLUS_PRIORITY_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for PriorityTier
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                UNCLAIMED_PRIORITY_INT => Ok(Self::Unclaimed),
                FREE_PRIORITY_INT => Ok(Self::Free),
                PLUS_PRIORITY_INT => Ok(Self::Plus),
                value => Err(Box::new(PriorityTierError::Invalid(value))),
            }
        }
    }
}
