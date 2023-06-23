use bencher_json::JsonMetric;
use diesel::RunQueryDsl;
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    model::project::threshold::{alert::InsertAlert, boundary::InsertBoundary},
    schema, ApiError,
};

mod boundary;
pub mod data;
pub mod threshold;

use boundary::Boundary;
use data::MetricsData;
use threshold::MetricsThreshold;

#[derive(Debug, Clone)]
pub struct Detector {
    pub branch_id: i32,
    pub testbed_id: i32,
    pub metric_kind_id: i32,
    pub threshold: MetricsThreshold,
}

impl Detector {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: i32,
        testbed_id: i32,
        metric_kind_id: i32,
    ) -> Result<Option<Self>, ApiError> {
        // Check to see if there is a threshold for the branch/testbed/metric kind grouping.
        // If not, then there will be nothing to detect.
        let threshold = if let Some(threshold) =
            MetricsThreshold::new(conn, branch_id, testbed_id, metric_kind_id)
        {
            threshold
        } else {
            return Ok(None);
        };

        Ok(Some(Self {
            branch_id,
            testbed_id,
            metric_kind_id,
            threshold,
        }))
    }

    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::float_arithmetic,
        clippy::integer_arithmetic
    )]
    pub fn detect(
        &self,
        conn: &mut DbConnection,
        perf_id: i32,
        benchmark_id: i32,
        metric: JsonMetric,
    ) -> Result<(), ApiError> {
        // Query the historical population/sample data for the benchmark
        let metrics_data = MetricsData::new(
            conn,
            self.branch_id,
            self.testbed_id,
            self.metric_kind_id,
            benchmark_id,
            &self.threshold.statistic,
        )?;

        // Check to see if the metric has a boundary check for the given threshold statistic.
        let Some(boundary) = Boundary::check(
            metric,
            metrics_data,
            self.threshold.statistic.test.try_into()?,
            self.threshold.statistic.min_sample_size,
            self.threshold.statistic.left_side,
            self.threshold.statistic.right_side,
        )? else {
            return Ok(());
        };

        let boundary_uuid = Uuid::new_v4();
        let insert_boundary = InsertBoundary {
            uuid: boundary_uuid.to_string(),
            perf_id,
            threshold_id: self.threshold.id,
            statistic_id: self.threshold.statistic.id,
            boundary_side: boundary.side.into(),
            boundary_limit: boundary.limit,
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)
            .map_err(api_error!())?;

        // If the boundary check detects an outlier then create an alert for it.
        if boundary.outlier {
            InsertAlert::from_boundary(conn, boundary_uuid)
        } else {
            Ok(())
        }
    }
}
