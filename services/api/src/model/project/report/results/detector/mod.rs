use bencher_boundary::MetricsBoundary;
use bencher_json::BoundaryUuid;
use diesel::RunQueryDsl;
use dropshot::HttpError;
use slog::Logger;

use crate::{
    context::DbConnection,
    error::{bad_request_error, resource_conflict_err},
    model::project::{
        benchmark::BenchmarkId,
        branch::BranchId,
        measure::MeasureId,
        metric::QueryMetric,
        testbed::TestbedId,
        threshold::{alert::InsertAlert, boundary::InsertBoundary},
    },
    schema,
};

pub mod data;
pub mod threshold;

use data::metrics_data;
use threshold::MetricsThreshold;

#[derive(Debug, Clone)]
pub struct Detector {
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
    pub threshold: MetricsThreshold,
}

impl Detector {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Option<Self> {
        // Check to see if there is a threshold for the branch/testbed/measure grouping.
        // If not, then there will be nothing to detect.
        MetricsThreshold::new(conn, branch_id, testbed_id, measure_id).map(|threshold| Self {
            branch_id,
            testbed_id,
            measure_id,
            threshold,
        })
    }

    pub fn detect(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        query_metric: &QueryMetric,
    ) -> Result<(), HttpError> {
        // Query the historical population/sample data for the benchmark
        let metrics_data = metrics_data(
            log,
            conn,
            self.branch_id,
            self.testbed_id,
            benchmark_id,
            self.measure_id,
            &self.threshold.statistic,
        )?;

        // Check to see if the metric has a boundary check for the given threshold statistic.
        let boundary = MetricsBoundary::new(
            log,
            query_metric.value,
            &metrics_data,
            self.threshold.statistic.test,
            self.threshold.statistic.min_sample_size,
            self.threshold.statistic.lower_boundary,
            self.threshold.statistic.upper_boundary,
        )
        .map_err(bad_request_error)?;

        let boundary_uuid = BoundaryUuid::new();
        let insert_boundary = InsertBoundary {
            uuid: boundary_uuid,
            threshold_id: self.threshold.id,
            statistic_id: self.threshold.statistic.id,
            metric_id: query_metric.id,
            baseline: boundary.limits.baseline,
            lower_limit: boundary.limits.lower.map(Into::into),
            upper_limit: boundary.limits.upper.map(Into::into),
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)
            .map_err(resource_conflict_err!(Boundary, insert_boundary))?;

        // If the boundary check detects an outlier then create an alert for it on the given side.
        if let Some(boundary_limit) = boundary.outlier {
            InsertAlert::from_boundary(conn, boundary_uuid, boundary_limit)
        } else {
            Ok(())
        }
    }
}
