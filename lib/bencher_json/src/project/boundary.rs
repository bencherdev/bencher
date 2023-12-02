use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(BoundaryUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBoundaries(pub Vec<JsonBoundary>);

crate::from_vec!(JsonBoundaries[JsonBoundary]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBoundary {
    pub lower_limit: Option<OrderedFloat<f64>>,
    pub upper_limit: Option<OrderedFloat<f64>>,
}

const LOWER_BOOL: bool = false;
const UPPER_BOOL: bool = true;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Bool))]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum BoundaryLimit {
    Lower = LOWER_BOOL as u8,
    Upper = UPPER_BOOL as u8,
}

#[cfg(feature = "db")]
mod boundary_limit {
    use super::{BoundaryLimit, LOWER_BOOL, UPPER_BOOL};

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Bool, DB> for BoundaryLimit
    where
        DB: diesel::backend::Backend,
        bool: diesel::serialize::ToSql<diesel::sql_types::Bool, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Lower => LOWER_BOOL.to_sql(out),
                Self::Upper => UPPER_BOOL.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Bool, DB> for BoundaryLimit
    where
        DB: diesel::backend::Backend,
        bool: diesel::deserialize::FromSql<diesel::sql_types::Bool, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            #[allow(clippy::match_bool)]
            match bool::from_sql(bytes)? {
                LOWER_BOOL => Ok(Self::Lower),
                UPPER_BOOL => Ok(Self::Upper),
            }
        }
    }
}
