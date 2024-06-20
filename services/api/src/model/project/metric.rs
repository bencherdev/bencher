use bencher_json::{JsonMetric, JsonNewMetric, MetricUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

#[cfg(feature = "plus")]
use crate::model::organization::OrganizationId;
use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema::{self, metric as metric_table},
};

use super::{
    measure::{MeasureId, QueryMeasure},
    report::report_benchmark::{QueryReportBenchmark, ReportBenchmarkId},
};

crate::util::typed_id::typed_id!(MetricId);

#[derive(
    Debug, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = metric_table)]
#[diesel(belongs_to(QueryReportBenchmark, foreign_key = report_benchmark_id))]
#[diesel(belongs_to(QueryMeasure, foreign_key = measure_id))]
pub struct QueryMetric {
    pub id: MetricId,
    pub uuid: MetricUuid,
    pub report_benchmark_id: ReportBenchmarkId,
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
        organization_id: OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
    ) -> Result<u32, HttpError> {
        schema::metric::table
            .inner_join(
                schema::report_benchmark::table
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

    pub fn into_json(self) -> JsonMetric {
        let Self {
            uuid,
            value,
            lower_value,
            upper_value,
            ..
        } = self;
        JsonMetric {
            uuid,
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
    pub report_benchmark_id: ReportBenchmarkId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
}

impl InsertMetric {
    pub fn from_json(
        report_benchmark_id: ReportBenchmarkId,
        measure_id: MeasureId,
        metric: JsonNewMetric,
    ) -> Self {
        let JsonNewMetric {
            value,
            lower_value,
            upper_value,
        } = metric;
        Self {
            uuid: MetricUuid::new(),
            report_benchmark_id,
            measure_id,
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }
}
