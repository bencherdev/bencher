use bencher_json::{BoundaryUuid, MetricUuid};

use crate::{model::project::metric::MetricId, view::metric_boundary as metric_boundary_table};

use super::{
    measure::{MeasureId, QueryMeasure},
    metric::QueryMetric,
    report::report_benchmark::{QueryReportBenchmark, ReportBenchmarkId},
    threshold::{
        boundary::{BoundaryId, QueryBoundary},
        model::ModelId,
        ThresholdId,
    },
};

// This is a materialized view of the metric and boundary tables
// It is an optimization to speed up the query time performance for perf queries
#[derive(
    Debug, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = metric_boundary_table)]
#[diesel(primary_key(metric_id))]
#[diesel(belongs_to(QueryReportBenchmark, foreign_key = report_benchmark_id))]
#[diesel(belongs_to(QueryMeasure, foreign_key = measure_id))]
pub struct QueryMetricBoundary {
    pub metric_id: MetricId,
    pub metric_uuid: MetricUuid,
    pub report_benchmark_id: ReportBenchmarkId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
    pub boundary_id: Option<BoundaryId>,
    pub boundary_uuid: Option<BoundaryUuid>,
    pub threshold_id: Option<ThresholdId>,
    pub model_id: Option<ModelId>,
    pub baseline: Option<f64>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}

impl QueryMetricBoundary {
    pub fn split(self) -> (QueryMetric, Option<QueryBoundary>) {
        let Self {
            metric_id,
            metric_uuid,
            report_benchmark_id,
            measure_id,
            value,
            lower_value,
            upper_value,
            boundary_id,
            boundary_uuid,
            threshold_id,
            model_id,
            baseline,
            lower_limit,
            upper_limit,
        } = self;
        let query_metric = QueryMetric {
            id: metric_id,
            uuid: metric_uuid,
            report_benchmark_id,
            measure_id,
            value,
            lower_value,
            upper_value,
        };
        let query_boundary = if let (Some(id), Some(uuid), Some(threshold_id), Some(model_id)) =
            (boundary_id, boundary_uuid, threshold_id, model_id)
        {
            Some(QueryBoundary {
                id,
                uuid,
                metric_id,
                threshold_id,
                model_id,
                baseline,
                lower_limit,
                upper_limit,
            })
        } else {
            None
        };
        (query_metric, query_boundary)
    }
}
