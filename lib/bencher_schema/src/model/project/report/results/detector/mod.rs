use bencher_boundary::MetricsBoundary;
use bencher_json::{
    AlertUuid, BoundaryUuid, project::alert::AlertStatus, project::boundary::BoundaryLimit,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    error::{bad_request_error, resource_conflict_err},
    model::project::{
        benchmark::BenchmarkId,
        branch::{BranchId, head::HeadId},
        measure::MeasureId,
        metric::MetricId,
        testbed::TestbedId,
        threshold::{
            ThresholdId,
            alert::InsertAlert,
            boundary::{BoundaryId, InsertBoundary},
            model::ModelId,
        },
    },
    schema,
};

pub mod data;
pub mod threshold;

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

/// Pre-computed detection result from Phase 1 (reads + compute).
/// Contains all data needed to write boundary and optional alert in Phase 2.
pub struct PreparedDetection {
    pub threshold_id: ThresholdId,
    pub model_id: ModelId,
    pub boundary_uuid: BoundaryUuid,
    pub baseline: Option<f64>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
    pub outlier: Option<BoundaryLimit>,
    pub ignore_benchmark: bool,
}

impl PreparedDetection {
    /// Write this prepared detection (boundary + optional alert) into the database
    /// using the provided connection (expected to be within a transaction).
    pub fn write(self, conn: &mut DbConnection, metric_id: MetricId) -> Result<(), HttpError> {
        let insert_boundary = InsertBoundary {
            uuid: self.boundary_uuid,
            threshold_id: self.threshold_id,
            model_id: self.model_id,
            metric_id,
            baseline: self.baseline,
            lower_limit: self.lower_limit,
            upper_limit: self.upper_limit,
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)
            .map_err(resource_conflict_err!(Boundary, insert_boundary))?;

        let boundary_id = schema::boundary::table
            .filter(schema::boundary::uuid.eq(self.boundary_uuid.to_string()))
            .select(schema::boundary::id)
            .first::<BoundaryId>(conn)
            .map_err(|e| {
                let message = format!(
                    "Failed to query boundary table for ID with UUID ({})",
                    self.boundary_uuid,
                );
                crate::error::issue_error(&message, &message, e)
            })?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::MetricCreate);

        if !self.ignore_benchmark && let Some(boundary_limit) = self.outlier {
            let insert_alert = InsertAlert {
                uuid: AlertUuid::new(),
                boundary_id,
                boundary_limit,
                status: AlertStatus::default(),
                modified: bencher_json::DateTime::now(),
            };
            diesel::insert_into(schema::alert::table)
                .values(&insert_alert)
                .execute(conn)
                .map_err(resource_conflict_err!(Alert, insert_alert))?;
        }

        Ok(())
    }
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
    use crate::model::project::{
        branch::{BranchId, head::HeadId},
        measure::MeasureId,
        testbed::TestbedId,
        threshold::ThresholdId,
    };

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
            BranchId::from_raw(branch.branch_id),
            HeadId::from_raw(branch.head_id),
            TestbedId::from_raw(testbed),
            None,
            MeasureId::from_raw(measure),
        );
        assert!(detector.is_none());
    }

    #[test]
    fn test_detector_new_returns_some_with_threshold() {
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
            BranchId::from_raw(branch.branch_id),
            HeadId::from_raw(branch.head_id),
            TestbedId::from_raw(testbed),
            None,
            MeasureId::from_raw(measure),
        );
        assert!(detector.is_some());
        let detector = detector.unwrap();
        assert_eq!(
            i32::from(detector.threshold.id),
            i32::from(ThresholdId::from_raw(threshold_id))
        );
    }
}
