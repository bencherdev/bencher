use bencher_json::{JsonMetric, MetricUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema::{self, metric as metric_table},
};

use super::{
    measure::{MeasureId, QueryMeasure},
    perf::{PerfId, QueryPerf},
};

crate::util::typed_id::typed_id!(MetricId);

#[derive(
    Debug, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = metric_table)]
#[diesel(belongs_to(QueryPerf, foreign_key = perf_id))]
#[diesel(belongs_to(QueryMeasure, foreign_key = measure_id))]
pub struct QueryMetric {
    pub id: MetricId,
    pub uuid: MetricUuid,
    pub perf_id: PerfId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
}

impl QueryMetric {
    pub fn from_uuid(conn: &mut DbConnection, uuid: MetricUuid) -> Result<Self, HttpError> {
        schema::metric::table
            .filter(schema::metric::uuid.eq(uuid))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Metric, uuid))
    }

    #[cfg(feature = "plus")]
    pub fn usage(
        conn: &mut DbConnection,
        organization_id: crate::model::organization::OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
    ) -> Result<u32, HttpError> {
        schema::metric::table
            .inner_join(
                schema::perf::table
                    .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                    .inner_join(schema::report::table),
            )
            .filter(schema::project::organization_id.eq(organization_id))
            .filter(schema::report::end_time.ge(start_time))
            .filter(schema::report::end_time.le(end_time))
            .select(diesel::dsl::count(schema::metric::id))
            .first::<i64>(conn)
            .map_err(resource_not_found_err!(Metric, (organization_id, start_time, end_time)))?
            .try_into()
            .map_err(|e| {
                crate::error::issue_error(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to count metric usage",
                    &format!("Failed to count metric usage for organization ({organization_id}) between {start_time} and {end_time}."),
                e
                )}
            )
    }

    pub fn json(value: f64, lower_value: Option<f64>, upper_value: Option<f64>) -> JsonMetric {
        JsonMetric {
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }

    pub fn into_json(self) -> JsonMetric {
        let Self {
            value,
            lower_value,
            upper_value,
            ..
        } = self;
        JsonMetric {
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = metric_table)]
pub struct InsertMetric {
    pub uuid: MetricUuid,
    pub perf_id: PerfId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
}

impl InsertMetric {
    pub fn from_json(perf_id: PerfId, measure_id: MeasureId, metric: JsonMetric) -> Self {
        let JsonMetric {
            value,
            lower_value,
            upper_value,
        } = metric;
        Self {
            uuid: MetricUuid::new(),
            perf_id,
            measure_id,
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }
}
