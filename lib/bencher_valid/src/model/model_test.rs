#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

const STATIC_INT: i32 = 20;
const PERCENTAGE_INT: i32 = 30;
const Z_SCORE_INT: i32 = 1;
const T_TEST_INT: i32 = 0;
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
        DELTA_IQR_INT, IQR_INT, LOG_NORMAL_INT, ModelTest, PERCENTAGE_INT, STATIC_INT, T_TEST_INT,
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
                Self::ZScore => Z_SCORE_INT.to_sql(out),
                Self::TTest => T_TEST_INT.to_sql(out),
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
                Z_SCORE_INT => Ok(Self::ZScore),
                T_TEST_INT => Ok(Self::TTest),
                LOG_NORMAL_INT => Ok(Self::LogNormal),
                IQR_INT => Ok(Self::Iqr),
                DELTA_IQR_INT => Ok(Self::DeltaIqr),
                value => Err(Box::new(ModelTestError::Invalid(value))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        DELTA_IQR_INT, IQR_INT, LOG_NORMAL_INT, ModelTest, PERCENTAGE_INT, STATIC_INT, T_TEST_INT,
        Z_SCORE_INT,
    };

    pub(super) const ALL_MODEL_TESTS: [(ModelTest, i32); 7] = [
        (ModelTest::Static, STATIC_INT),
        (ModelTest::Percentage, PERCENTAGE_INT),
        (ModelTest::ZScore, Z_SCORE_INT),
        (ModelTest::TTest, T_TEST_INT),
        (ModelTest::LogNormal, LOG_NORMAL_INT),
        (ModelTest::Iqr, IQR_INT),
        (ModelTest::DeltaIqr, DELTA_IQR_INT),
    ];

    #[test]
    fn model_test_i32_constants() {
        assert_eq!(20, STATIC_INT);
        assert_eq!(30, PERCENTAGE_INT);
        assert_eq!(1, Z_SCORE_INT);
        assert_eq!(0, T_TEST_INT);
        assert_eq!(10, LOG_NORMAL_INT);
        assert_eq!(40, IQR_INT);
        assert_eq!(41, DELTA_IQR_INT);
    }

    #[test]
    fn model_test_i32_repr() {
        for (model_test, expected) in ALL_MODEL_TESTS {
            assert_eq!(expected, model_test as i32, "{model_test}");
        }
    }

    #[test]
    fn model_test_serde_snake_case() {
        let json_cases = [
            (ModelTest::Static, "\"static\""),
            (ModelTest::Percentage, "\"percentage\""),
            (ModelTest::ZScore, "\"z_score\""),
            (ModelTest::TTest, "\"t_test\""),
            (ModelTest::LogNormal, "\"log_normal\""),
            (ModelTest::Iqr, "\"iqr\""),
            (ModelTest::DeltaIqr, "\"delta_iqr\""),
        ];
        for (model_test, json) in json_cases {
            assert_eq!(
                json,
                serde_json::to_string(&model_test).expect("serialize model test"),
                "{model_test}"
            );
            assert_eq!(
                model_test,
                serde_json::from_str::<ModelTest>(json).expect("deserialize model test"),
                "{json}"
            );
        }
    }

    #[test]
    fn model_test_serde_aliases() {
        assert_eq!(
            ModelTest::ZScore,
            serde_json::from_str::<ModelTest>("\"z\"").expect("z alias")
        );
        assert_eq!(
            ModelTest::TTest,
            serde_json::from_str::<ModelTest>("\"t\"").expect("t alias")
        );
    }

    #[test]
    fn model_test_serde_invalid() {
        serde_json::from_str::<ModelTest>("\"bogus\"").unwrap_err();
        serde_json::from_str::<ModelTest>("\"zscore\"").unwrap_err();
        // The serde representation is the snake case string, not the integer.
        serde_json::from_str::<ModelTest>("20").unwrap_err();
    }
}

#[cfg(test)]
#[cfg(feature = "db")]
mod db_tests {
    use diesel::{Connection as _, IntoSql as _, RunQueryDsl as _, SqliteConnection};
    use pretty_assertions::assert_eq;

    use super::{ModelTest, tests::ALL_MODEL_TESTS};

    fn connection() -> SqliteConnection {
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database")
    }

    #[test]
    fn model_test_to_sql() {
        let mut conn = connection();
        for (model_test, expected) in ALL_MODEL_TESTS {
            let value: i32 = diesel::select(model_test.into_sql::<diesel::sql_types::Integer>())
                .get_result(&mut conn)
                .expect("Failed to select ModelTest as i32");
            assert_eq!(expected, value, "{model_test}");
        }
    }

    #[test]
    fn model_test_from_sql() {
        let mut conn = connection();
        for (expected, value) in ALL_MODEL_TESTS {
            let model_test: ModelTest =
                diesel::select(value.into_sql::<diesel::sql_types::Integer>())
                    .get_result(&mut conn)
                    .expect("Failed to select i32 as ModelTest");
            assert_eq!(expected, model_test, "{value}");
        }
    }

    #[test]
    fn model_test_from_sql_invalid() {
        let mut conn = connection();
        for invalid in [i32::MIN, -1, 2, 11, 21, 31, 42, 99, i32::MAX] {
            let error = diesel::select(invalid.into_sql::<diesel::sql_types::Integer>())
                .get_result::<ModelTest>(&mut conn)
                .expect_err("Invalid i32 should not deserialize to ModelTest");
            let message = error.to_string();
            assert!(
                message.contains(&format!("Invalid model kind value: {invalid}")),
                "{message}"
            );
        }
    }
}
