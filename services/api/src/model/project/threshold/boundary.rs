use std::str::FromStr;

use bencher_json::project::boundary::JsonBoundary;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    model::project::metric::MetricId,
    schema,
    schema::boundary as boundary_table,
    util::query::{fn_get, fn_get_id},
    ApiError,
};

use super::{statistic::StatisticId, ThresholdId};

crate::util::typed_id::typed_id!(BoundaryId);

#[derive(Queryable)]
pub struct QueryBoundary {
    pub id: BoundaryId,
    pub uuid: String,
    pub threshold_id: ThresholdId,
    pub statistic_id: StatisticId,
    pub metric_id: MetricId,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}

impl QueryBoundary {
    fn_get!(boundary);
    fn_get_id!(boundary, BoundaryId);

    pub fn get_uuid(conn: &mut DbConnection, id: BoundaryId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::alert::table
            .filter(schema::alert::id.eq(id))
            .select(schema::alert::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }

    pub fn from_metric_id(conn: &mut DbConnection, metric_id: MetricId) -> Result<Self, ApiError> {
        schema::boundary::table
            .filter(schema::boundary::metric_id.eq(metric_id))
            .first::<Self>(conn)
            .map_err(ApiError::from)
    }

    // There may not be a boundary for every metric, so return the default if there isn't one.
    pub fn get_json(conn: &mut DbConnection, metric_id: MetricId) -> JsonBoundary {
        Self::from_metric_id(conn, metric_id)
            .map(QueryBoundary::into_json)
            .unwrap_or_default()
    }

    pub fn into_json(self) -> JsonBoundary {
        JsonBoundary {
            lower_limit: self.lower_limit.map(Into::into),
            upper_limit: self.upper_limit.map(Into::into),
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = boundary_table)]
pub struct InsertBoundary {
    pub uuid: String,
    pub threshold_id: ThresholdId,
    pub statistic_id: StatisticId,
    pub metric_id: MetricId,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}
