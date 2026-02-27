use bencher_boundary::MetricsBoundary;
use bencher_json::BoundaryUuid;
use dropshot::HttpError;
use slog::Logger;

use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    error::bad_request_error,
    model::project::{
        benchmark::BenchmarkId,
        branch::{BranchId, head::HeadId},
        measure::MeasureId,
        testbed::TestbedId,
    },
};

pub mod data;
mod prepared;
pub mod threshold;

pub use prepared::PreparedDetection;

use data::metrics_data;
use threshold::Threshold;

#[derive(Debug, Clone)]
pub struct Detector {
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub measure_id: MeasureId,
    pub threshold: Threshold,
}

impl Detector {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: BranchId,
        head_id: HeadId,
        testbed_id: TestbedId,
        spec_id: Option<SpecId>,
        measure_id: MeasureId,
    ) -> Option<Self> {
        // Check to see if there is a threshold for the branch/testbed/measure grouping.
        // If not, then there will be nothing to detect.
        Threshold::new(conn, branch_id, testbed_id, measure_id).map(|threshold| Self {
            head_id,
            testbed_id,
            spec_id,
            measure_id,
            threshold,
        })
    }

    /// Phase 1: Read historical data and compute the boundary.
    /// Returns a `PreparedDetection` that can be written in Phase 2.
    pub fn prepare_detection(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        metric_value: f64,
        ignore_benchmark: bool,
    ) -> Result<PreparedDetection, HttpError> {
        // Query the historical population/sample data for the benchmark
        let metrics_data = metrics_data(log, conn, self, benchmark_id)?;

        // Check to see if the metric has a boundary check for the given threshold model.
        let boundary = MetricsBoundary::new(
            log,
            metric_value,
            &metrics_data,
            self.threshold.model.test,
            self.threshold.model.min_sample_size,
            self.threshold.model.lower_boundary,
            self.threshold.model.upper_boundary,
        )
        .map_err(bad_request_error)?;

        Ok(PreparedDetection {
            threshold_id: self.threshold.id,
            model_id: self.threshold.model.id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: boundary.limits.baseline,
            lower_limit: boundary.limits.lower.map(Into::into),
            upper_limit: boundary.limits.upper.map(Into::into),
            outlier: boundary.outlier,
            ignore_benchmark,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::{
        create_base_entities, create_branch_with_head, create_measure, create_model,
        create_testbed, create_threshold, setup_test_db,
    };

    use super::Detector;

    #[test]
    fn detector_new_returns_none_without_threshold() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // No threshold exists => Detector::new returns None
        let detector = Detector::new(
            &mut conn,
            branch.branch_id,
            branch.head_id,
            testbed,
            None,
            measure,
        );
        assert!(detector.is_none());
    }

    #[test]
    fn detector_new_returns_some_with_threshold() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );
        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );

        // Threshold + model exist => Detector::new returns Some
        let detector = Detector::new(
            &mut conn,
            branch.branch_id,
            branch.head_id,
            testbed,
            None,
            measure,
        );
        assert!(detector.is_some());
        let detector = detector.unwrap();
        assert_eq!(detector.threshold.id, threshold_id);
    }
}
