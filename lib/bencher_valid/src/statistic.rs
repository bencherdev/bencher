#[cfg(feature = "schema")]
use schemars::JsonSchema;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{Deserialize, Serialize};

use crate::{
    boundary::{CdfBoundary, IqrBoundary, PercentageBoundary},
    Boundary, SampleSize, ValidError, Window,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Statistic {
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl Statistic {
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

    pub fn validate(self) -> Result<(), ValidError> {
        validate_statistic(self)
    }
}

pub fn validate_statistic(statistic: Statistic) -> Result<(), ValidError> {
    let Statistic {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
    } = statistic;
    match test {
        StatisticKind::Static => {
            if let Some(&min_sample_size) = min_sample_size.as_ref() {
                return Err(ValidError::StaticMinSampleSize(min_sample_size));
            } else if let Some(&max_sample_size) = max_sample_size.as_ref() {
                return Err(ValidError::StaticMaxSampleSize(max_sample_size));
            } else if let Some(&window) = window.as_ref() {
                return Err(ValidError::StaticWindow(window));
            }

            match (lower_boundary.as_ref(), upper_boundary.as_ref()) {
                (Some(&lower), Some(&upper)) => {
                    if f64::from(lower) > f64::from(upper) {
                        Err(ValidError::Boundaries { lower, upper })
                    } else {
                        Ok(())
                    }
                },
                (Some(_), None) | (None, Some(_)) => Ok(()),
                (None, None) => Err(ValidError::NoBoundary),
            }
        },
        StatisticKind::Percentage => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<PercentageBoundary>(lower_boundary, upper_boundary)
        },
        StatisticKind::ZScore | StatisticKind::TTest | StatisticKind::LogNormal => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<CdfBoundary>(lower_boundary, upper_boundary)
        },
        StatisticKind::Iqr | StatisticKind::DeltaIqr => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<IqrBoundary>(lower_boundary, upper_boundary)
        },
    }
}

fn validate_sample_size(
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
) -> Result<(), ValidError> {
    if let (Some(min), Some(max)) = (min_sample_size, max_sample_size) {
        if u32::from(min) > u32::from(max) {
            return Err(ValidError::SampleSizes { min, max });
        }
    }

    Ok(())
}

fn validate_boundary<B>(lower: Option<Boundary>, upper: Option<Boundary>) -> Result<(), ValidError>
where
    B: TryFrom<Boundary, Error = ValidError>,
    f64: From<B>,
{
    match (lower, upper) {
        (Some(lower), Some(upper)) => {
            B::try_from(lower)?;
            B::try_from(upper)?;
            Ok(())
        },
        (Some(boundary), None) | (None, Some(boundary)) => {
            B::try_from(boundary)?;
            Ok(())
        },
        (None, None) => Err(ValidError::NoBoundary),
    }
}

#[cfg(feature = "wasm")]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_statistic(statistic: &str) -> bool {
    let Ok(statistic) = serde_json::from_str(statistic) else {
        return false;
    };
    validate_statistic(statistic).is_ok()
}

const Z_SCORE_INT: i32 = 0;
const T_TEST_INT: i32 = 1;
const STATIC_INT: i32 = 2;
const PERCENTAGE_INT: i32 = 3;
const LOG_NORMAL_INT: i32 = 4;
const IQR_INT: i32 = 5;
const DELTA_IQR_INT: i32 = 6;

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
    LogNormal = LOG_NORMAL_INT,
    Iqr = IQR_INT,
    DeltaIqr = DELTA_IQR_INT,
}

#[cfg(feature = "db")]
mod statistic_kind {
    use super::{
        StatisticKind, DELTA_IQR_INT, IQR_INT, LOG_NORMAL_INT, PERCENTAGE_INT, STATIC_INT,
        T_TEST_INT, Z_SCORE_INT,
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
                Self::LogNormal => LOG_NORMAL_INT.to_sql(out),
                Self::Iqr => IQR_INT.to_sql(out),
                Self::DeltaIqr => DELTA_IQR_INT.to_sql(out),
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
                LOG_NORMAL_INT => Ok(Self::LogNormal),
                IQR_INT => Ok(Self::Iqr),
                DELTA_IQR_INT => Ok(Self::DeltaIqr),
                value => Err(Box::new(StatisticKindError::Invalid(value))),
            }
        }
    }
}
