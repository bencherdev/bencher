use std::str::FromStr;

use bencher_json::{
    project::threshold::{JsonNewStatistic, JsonStatistic, JsonStatisticKind},
    Boundary, SampleSize,
};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    schema,
    schema::statistic as statistic_table,
    util::{
        map_u32,
        query::{fn_get, fn_get_id},
        to_date_time,
    },
    ApiError,
};

use super::{QueryThreshold, ThresholdId};

crate::util::typed_id::typed_id!(StatisticId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = statistic_table)]
pub struct QueryStatistic {
    pub id: StatisticId,
    pub uuid: String,
    pub threshold_id: ThresholdId,
    pub test: StatisticKind,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub lower_boundary: Option<f64>,
    pub upper_boundary: Option<f64>,
    pub created: i64,
}

impl QueryStatistic {
    fn_get!(statistic);
    fn_get_id!(statistic, StatisticId);

    pub fn get_uuid(conn: &mut DbConnection, id: StatisticId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::statistic::table
            .filter(schema::statistic::id.eq(id))
            .select(schema::statistic::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonStatistic, ApiError> {
        let threshold = QueryThreshold::get_uuid(conn, self.threshold_id)?;
        self.into_json_for_threshold(threshold)
    }

    pub fn into_json_for_threshold(self, threshold: Uuid) -> Result<JsonStatistic, ApiError> {
        let Self {
            uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            ..
        } = self;
        Ok(JsonStatistic {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            threshold,
            test: test.into(),
            min_sample_size: map_sample_size(min_sample_size)?,
            max_sample_size: map_sample_size(max_sample_size)?,
            window: map_u32(window)?,
            lower_boundary: map_boundary(lower_boundary)?,
            upper_boundary: map_boundary(upper_boundary)?,
            created: to_date_time(created).map_err(ApiError::from)?,
        })
    }
}

pub fn map_sample_size(sample_size: Option<i64>) -> Result<Option<SampleSize>, ApiError> {
    Ok(if let Some(sample_size) = sample_size {
        Some(u32::try_from(sample_size)?.try_into()?)
    } else {
        None
    })
}

const Z_INT: i32 = 0;
const T_INT: i32 = 1;

#[derive(Debug, Clone, Copy, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
#[repr(i32)]
pub enum StatisticKind {
    Z = Z_INT,
    T = T_INT,
}

impl TryFrom<i32> for StatisticKind {
    type Error = ApiError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        match kind {
            Z_INT => Ok(Self::Z),
            T_INT => Ok(Self::T),
            _ => Err(ApiError::StatisticKind(kind)),
        }
    }
}

impl From<JsonStatisticKind> for StatisticKind {
    fn from(kind: JsonStatisticKind) -> Self {
        match kind {
            JsonStatisticKind::Z => Self::Z,
            JsonStatisticKind::T => Self::T,
        }
    }
}

impl From<StatisticKind> for JsonStatisticKind {
    fn from(kind: StatisticKind) -> Self {
        match kind {
            StatisticKind::Z => Self::Z,
            StatisticKind::T => Self::T,
        }
    }
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
            Self::Z => Z_INT.to_sql(out),
            Self::T => T_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for StatisticKind
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self::try_from(i32::from_sql(bytes)?)?)
    }
}

pub fn map_boundary(boundary: Option<f64>) -> Result<Option<Boundary>, ApiError> {
    Ok(if let Some(boundary) = boundary {
        Some(boundary.try_into()?)
    } else {
        None
    })
}

#[derive(diesel::Insertable)]
#[diesel(table_name = statistic_table)]
pub struct InsertStatistic {
    pub uuid: String,
    pub threshold_id: ThresholdId,
    pub test: StatisticKind,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub lower_boundary: Option<f64>,
    pub upper_boundary: Option<f64>,
    pub created: i64,
}

impl From<QueryStatistic> for InsertStatistic {
    fn from(query_statistic: QueryStatistic) -> Self {
        let QueryStatistic {
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            ..
        } = query_statistic;
        Self {
            uuid: Uuid::new_v4().to_string(),
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
        }
    }
}

impl InsertStatistic {
    pub fn from_json(
        threshold_id: ThresholdId,
        json_statistic: JsonNewStatistic,
    ) -> Result<Self, ApiError> {
        let JsonNewStatistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = json_statistic;
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            threshold_id,
            test: StatisticKind::from(test),
            min_sample_size: min_sample_size.map(|ss| u32::from(ss).into()),
            max_sample_size: max_sample_size.map(|ss| u32::from(ss).into()),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
            created: Utc::now().timestamp(),
        })
    }
}
