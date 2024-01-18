use bencher_json::{project::boundary::JsonBoundary, BoundaryUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    model::project::metric::MetricId,
    schema,
    schema::boundary as boundary_table,
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{statistic::StatisticId, ThresholdId};

crate::util::typed_id::typed_id!(BoundaryId);

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = boundary_table)]
pub struct QueryBoundary {
    pub id: BoundaryId,
    pub uuid: BoundaryUuid,
    pub threshold_id: ThresholdId,
    pub statistic_id: StatisticId,
    pub metric_id: MetricId,
    pub baseline: Option<f64>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}

impl QueryBoundary {
    fn_get!(boundary, BoundaryId);
    fn_get_id!(boundary, BoundaryId, BoundaryUuid);
    fn_get_uuid!(boundary, BoundaryId, BoundaryUuid);

    pub fn from_metric_id(conn: &mut DbConnection, metric_id: MetricId) -> Result<Self, HttpError> {
        schema::boundary::table
            .filter(schema::boundary::metric_id.eq(metric_id))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Boundary, metric_id))
    }

    pub fn into_json(self) -> JsonBoundary {
        JsonBoundary {
            baseline: self.baseline.map(Into::into),
            lower_limit: self.lower_limit.map(Into::into),
            upper_limit: self.upper_limit.map(Into::into),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = boundary_table)]
pub struct InsertBoundary {
    pub uuid: BoundaryUuid,
    pub threshold_id: ThresholdId,
    pub statistic_id: StatisticId,
    pub metric_id: MetricId,
    pub baseline: Option<f64>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}
