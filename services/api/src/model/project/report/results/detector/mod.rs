use diesel::RunQueryDsl;
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    model::project::{
        metric::QueryMetric,
        threshold::{alert::InsertAlert, boundary::InsertBoundary},
    },
    schema, ApiError,
};

mod boundary;
pub mod data;
mod limits;
pub mod threshold;

use boundary::MetricsBoundary;
use data::MetricsData;
use threshold::MetricsThreshold;

#[derive(Debug, Clone)]
pub struct Detector {
    pub metric_kind_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub threshold: MetricsThreshold,
}

impl Detector {
    pub fn new(
        conn: &mut DbConnection,
        metric_kind_id: i32,
        branch_id: i32,
        testbed_id: i32,
    ) -> Result<Option<Self>, ApiError> {
        // Check to see if there is a threshold for the branch/testbed/metric kind grouping.
        // If not, then there will be nothing to detect.
        let threshold = if let Some(threshold) =
            MetricsThreshold::new(conn, metric_kind_id, branch_id, testbed_id)
        {
            threshold
        } else {
            return Ok(None);
        };

        Ok(Some(Self {
            metric_kind_id,
            branch_id,
            testbed_id,
            threshold,
        }))
    }

    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::float_arithmetic
    )]
    pub fn detect(
        &self,
        conn: &mut DbConnection,
        benchmark_id: i32,
        query_metric: QueryMetric,
    ) -> Result<(), ApiError> {
        // Query the historical population/sample data for the benchmark
        let metrics_data = MetricsData::new(
            conn,
            self.metric_kind_id,
            self.branch_id,
            self.testbed_id,
            benchmark_id,
            &self.threshold.statistic,
        )?;

        // Check to see if the metric has a boundary check for the given threshold statistic.
        let boundary = MetricsBoundary::new(
            query_metric.value,
            metrics_data,
            self.threshold.statistic.test,
            self.threshold.statistic.min_sample_size,
            self.threshold.statistic.lower_boundary,
            self.threshold.statistic.upper_boundary,
        )?;

        let boundary_uuid = Uuid::new_v4();
        let insert_boundary = InsertBoundary {
            uuid: boundary_uuid.to_string(),
            threshold_id: self.threshold.id,
            statistic_id: self.threshold.statistic.id,
            metric_id: query_metric.id,
            lower_limit: boundary.limits.lower.map(Into::into),
            upper_limit: boundary.limits.upper.map(Into::into),
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)
            .map_err(api_error!())?;

        // If the boundary check detects an outlier then create an alert for it on the given side.
        if let Some(side) = boundary.outlier {
            InsertAlert::from_boundary(conn, boundary_uuid, side)
        } else {
            Ok(())
        }
    }
}
