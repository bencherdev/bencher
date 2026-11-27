#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use crate::ValidError;

const STATIC_INT: i32 = 20;
const PERCENTAGE_INT: i32 = 30;
const Z_SCORE_INT: i32 = 1;
const T_TEST_INT: i32 = 0;
const LOG_NORMAL_INT: i32 = 10;
const IQR_INT: i32 = 40;
const DELTA_IQR_INT: i32 = 41;
const CHANGEPOINT_INT: i32 = 50;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum ModelTest {
    Static = STATIC_INT,
    Percentage = PERCENTAGE_INT,
    #[serde(alias = "z")]
    ZScore = Z_SCORE_INT,
    #[serde(alias = "t")]
    TTest = T_TEST_INT,
    LogNormal = LOG_NORMAL_INT,
    Iqr = IQR_INT,
    DeltaIqr = DELTA_IQR_INT,
    Changepoint = CHANGEPOINT_INT,
}

impl ModelTest {
    /// The slot of a threshold that a model with this test belongs in.
    #[must_use]
    pub const fn slot(self) -> ModelSlot {
        match self {
            Self::Static
            | Self::Percentage
            | Self::ZScore
            | Self::TTest
            | Self::LogNormal
            | Self::Iqr
            | Self::DeltaIqr => ModelSlot::Model,
            Self::Changepoint => ModelSlot::ChangepointModel,
        }
    }

    /// Whether a model with this test takes a significance, which it then requires.
    #[must_use]
    pub const fn takes_significance(self) -> bool {
        match self {
            Self::Changepoint => true,
            Self::Static
            | Self::Percentage
            | Self::ZScore
            | Self::TTest
            | Self::LogNormal
            | Self::Iqr
            | Self::DeltaIqr => false,
        }
    }

    /// Whether a model with this test takes a count of confirming scans.
    #[must_use]
    pub const fn takes_confirmations(self) -> bool {
        match self {
            Self::Changepoint => true,
            Self::Static
            | Self::Percentage
            | Self::ZScore
            | Self::TTest
            | Self::LogNormal
            | Self::Iqr
            | Self::DeltaIqr => false,
        }
    }
}

/// One of the two model slots of a threshold.
///
/// Which slot a model belongs in follows from its test, and a caller never states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display)]
pub enum ModelSlot {
    /// `model`, which holds a model whose test checks a metric.
    #[display("model")]
    Model,
    /// `changepoint_model`, which holds a model whose test checks a line.
    #[display("changepoint_model")]
    ChangepointModel,
}

impl ModelSlot {
    pub const ALL: [Self; 2] = [Self::Model, Self::ChangepointModel];

    /// Refuse a model test that belongs in the other slot.
    pub fn admit(self, test: ModelTest) -> Result<(), ValidError> {
        if test.slot() == self {
            Ok(())
        } else {
            Err(ValidError::ModelSlot { test, slot: self })
        }
    }

    /// What a model in this slot checks.
    #[must_use]
    pub const fn checks(self) -> &'static str {
        match self {
            Self::Model => "a metric",
            Self::ChangepointModel => "a line",
        }
    }
}

#[cfg(feature = "db")]
mod db {
    use super::{
        CHANGEPOINT_INT, DELTA_IQR_INT, IQR_INT, LOG_NORMAL_INT, ModelTest, PERCENTAGE_INT,
        STATIC_INT, T_TEST_INT, Z_SCORE_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum ModelTestError {
        #[error("Invalid model kind value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for ModelTest
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Static => STATIC_INT.to_sql(out),
                Self::Percentage => PERCENTAGE_INT.to_sql(out),
                Self::ZScore => Z_SCORE_INT.to_sql(out),
                Self::TTest => T_TEST_INT.to_sql(out),
                Self::LogNormal => LOG_NORMAL_INT.to_sql(out),
                Self::Iqr => IQR_INT.to_sql(out),
                Self::DeltaIqr => DELTA_IQR_INT.to_sql(out),
                Self::Changepoint => CHANGEPOINT_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for ModelTest
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                STATIC_INT => Ok(Self::Static),
                PERCENTAGE_INT => Ok(Self::Percentage),
                Z_SCORE_INT => Ok(Self::ZScore),
                T_TEST_INT => Ok(Self::TTest),
                LOG_NORMAL_INT => Ok(Self::LogNormal),
                IQR_INT => Ok(Self::Iqr),
                DELTA_IQR_INT => Ok(Self::DeltaIqr),
                CHANGEPOINT_INT => Ok(Self::Changepoint),
                value => Err(Box::new(ModelTestError::Invalid(value))),
            }
        }
    }
}
