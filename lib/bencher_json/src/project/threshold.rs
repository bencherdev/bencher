use bencher_valid::{Boundary, DateTime, SampleSize};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonBranch, JsonMetricKind, JsonTestbed, ProjectUuid, ResourceId};

crate::typed_uuid::typed_uuid!(ThresholdUuid);
crate::typed_uuid::typed_uuid!(StatisticUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewThreshold {
    pub metric_kind: ResourceId,
    pub branch: ResourceId,
    pub testbed: ResourceId,
    #[serde(flatten)]
    pub statistic: JsonNewStatistic,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewStatistic {
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<u32>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl JsonNewStatistic {
    pub fn lower_boundary() -> Self {
        Self {
            test: StatisticKind::T,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::THIRTY),
            window: None,
            lower_boundary: Some(Boundary::EIGHTY),
            upper_boundary: None,
        }
    }

    pub fn upper_boundary() -> Self {
        Self {
            test: StatisticKind::T,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::THIRTY),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::EIGHTY),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholds(pub Vec<JsonThreshold>);

crate::from_vec!(JsonThresholds[JsonThreshold]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThreshold {
    pub uuid: ThresholdUuid,
    pub project: ProjectUuid,
    pub metric_kind: JsonMetricKind,
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub statistic: JsonStatistic,
    pub created: DateTime,
    pub modified: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStatistic {
    pub uuid: StatisticUuid,
    pub threshold: ThresholdUuid,
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<u32>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub created: DateTime,
}

const Z_INT: i32 = 0;
const T_INT: i32 = 1;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum StatisticKind {
    Z = Z_INT,
    T = T_INT,
}

#[cfg(feature = "db")]
mod statistic_kind {
    use super::{StatisticKind, T_INT, Z_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum StatisticKindError {
        #[error("Invalid statistic kind value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for StatisticKind
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Z => T_INT.to_sql(out),
                Self::T => Z_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for StatisticKind
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                T_INT => Ok(Self::Z),
                Z_INT => Ok(Self::T),
                value => Err(Box::new(StatisticKindError::Invalid(value))),
            }
        }
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholdStatistic {
    pub uuid: ThresholdUuid,
    pub project: ProjectUuid,
    pub statistic: JsonStatistic,
    pub created: DateTime,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateThreshold {
    #[serde(flatten)]
    pub statistic: JsonNewStatistic,
}
