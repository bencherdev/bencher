#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

const STATIC_INT: i32 = 20;
const PERCENTAGE_INT: i32 = 30;
const Z_SCORE_INT: i32 = 0;
const T_TEST_INT: i32 = 1;
const LOG_NORMAL_INT: i32 = 10;
const IQR_INT: i32 = 40;
const DELTA_IQR_INT: i32 = 41;

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
}

#[cfg(feature = "db")]
mod db {
    use super::{
        ModelTest, DELTA_IQR_INT, IQR_INT, LOG_NORMAL_INT, PERCENTAGE_INT, STATIC_INT, T_TEST_INT,
        Z_SCORE_INT,
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
                Self::ZScore => T_TEST_INT.to_sql(out),
                Self::TTest => Z_SCORE_INT.to_sql(out),
                Self::LogNormal => LOG_NORMAL_INT.to_sql(out),
                Self::Iqr => IQR_INT.to_sql(out),
                Self::DeltaIqr => DELTA_IQR_INT.to_sql(out),
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
                T_TEST_INT => Ok(Self::ZScore),
                Z_SCORE_INT => Ok(Self::TTest),
                LOG_NORMAL_INT => Ok(Self::LogNormal),
                IQR_INT => Ok(Self::Iqr),
                DELTA_IQR_INT => Ok(Self::DeltaIqr),
                value => Err(Box::new(ModelTestError::Invalid(value))),
            }
        }
    }
}
