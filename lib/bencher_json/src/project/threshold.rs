use bencher_valid::{Boundary, DateTime, NameId, SampleSize, Window};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    urlencoded::{from_urlencoded, to_urlencoded, UrlEncodedError},
    JsonBranch, JsonMeasure, JsonTestbed, ProjectUuid,
};

crate::typed_uuid::typed_uuid!(ThresholdUuid);
crate::typed_uuid::typed_uuid!(StatisticUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewThreshold {
    pub branch: NameId,
    pub testbed: NameId,
    pub measure: NameId,
    #[serde(flatten)]
    pub statistic: JsonNewStatistic,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewStatistic {
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl JsonNewStatistic {
    pub fn lower_boundary() -> Self {
        Self {
            test: StatisticKind::TTest,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::TWO_FIFTY_FIVE),
            window: None,
            lower_boundary: Some(Boundary::NINETY_EIGHT),
            upper_boundary: None,
        }
    }

    pub fn upper_boundary() -> Self {
        Self {
            test: StatisticKind::TTest,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::TWO_FIFTY_FIVE),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::NINETY_EIGHT),
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
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub measure: JsonMeasure,
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
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub created: DateTime,
}

const Z_SCORE_INT: i32 = 0;
const T_TEST_INT: i32 = 1;
const STATIC_INT: i32 = 2;
const PERCENTAGE_INT: i32 = 3;
const IQR_INT: i32 = 4;
const LOG_NORMAL_INT: i32 = 5;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum StatisticKind {
    #[serde(alias = "z")]
    ZScore = Z_SCORE_INT,
    #[serde(alias = "t")]
    TTest = T_TEST_INT,
    Static = STATIC_INT,
    Percentage = PERCENTAGE_INT,
    IQR = IQR_INT,
    LogNormal = LOG_NORMAL_INT,
}

#[cfg(feature = "db")]
mod statistic_kind {
    use super::{
        StatisticKind, IQR_INT, LOG_NORMAL_INT, PERCENTAGE_INT, STATIC_INT, T_TEST_INT, Z_SCORE_INT,
    };

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
                Self::ZScore => T_TEST_INT.to_sql(out),
                Self::TTest => Z_SCORE_INT.to_sql(out),
                Self::Static => STATIC_INT.to_sql(out),
                Self::Percentage => PERCENTAGE_INT.to_sql(out),
                Self::IQR => IQR_INT.to_sql(out),
                Self::LogNormal => LOG_NORMAL_INT.to_sql(out),
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
                T_TEST_INT => Ok(Self::ZScore),
                Z_SCORE_INT => Ok(Self::TTest),
                STATIC_INT => Ok(Self::Static),
                PERCENTAGE_INT => Ok(Self::Percentage),
                IQR_INT => Ok(Self::IQR),
                LOG_NORMAL_INT => Ok(Self::LogNormal),
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

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholdQueryParams {
    pub branch: Option<String>,
    pub testbed: Option<String>,
    pub measure: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JsonThresholdQuery {
    pub branch: Option<NameId>,
    pub testbed: Option<NameId>,
    pub measure: Option<NameId>,
}

impl TryFrom<JsonThresholdQueryParams> for JsonThresholdQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonThresholdQueryParams) -> Result<Self, Self::Error> {
        let JsonThresholdQueryParams {
            branch,
            testbed,
            measure,
        } = query_params;

        let branch = if let Some(branch) = branch {
            Some(from_urlencoded(&branch)?)
        } else {
            None
        };
        let testbed = if let Some(testbed) = testbed {
            Some(from_urlencoded(&testbed)?)
        } else {
            None
        };
        let measure = if let Some(measure) = measure {
            Some(from_urlencoded(&measure)?)
        } else {
            None
        };

        Ok(Self {
            branch,
            testbed,
            measure,
        })
    }
}

impl JsonThresholdQuery {
    pub fn branch(&self) -> Option<String> {
        self.branch.as_ref().map(to_urlencoded)
    }

    pub fn testbed(&self) -> Option<String> {
        self.testbed.as_ref().map(to_urlencoded)
    }

    pub fn measure(&self) -> Option<String> {
        self.measure.as_ref().map(to_urlencoded)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateThreshold {
    #[serde(flatten)]
    pub statistic: JsonNewStatistic,
}
