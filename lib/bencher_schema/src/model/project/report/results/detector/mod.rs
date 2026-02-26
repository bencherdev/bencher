use bencher_boundary::MetricsBoundary;
use bencher_json::{BoundaryUuid, project::boundary::BoundaryLimit};
use diesel::RunQueryDsl as _;
use dropshot::HttpError;
use slog::Logger;

use crate::macros::sql::last_insert_rowid;
use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    error::bad_request_error,
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
    pub fn write(self, conn: &mut DbConnection, metric_id: MetricId) -> diesel::QueryResult<()> {
        let Self {
            threshold_id,
            model_id,
            boundary_uuid,
            baseline,
            lower_limit,
            upper_limit,
            outlier,
            ignore_benchmark,
        } = self;

        let insert_boundary = InsertBoundary {
            uuid: boundary_uuid,
            threshold_id,
            model_id,
            metric_id,
            baseline,
            lower_limit,
            upper_limit,
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)?;

        let boundary_id = diesel::select(last_insert_rowid()).get_result::<BoundaryId>(conn)?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::MetricCreate);

        if !ignore_benchmark && let Some(boundary_limit) = outlier {
            InsertAlert::insert(conn, boundary_id, boundary_limit)?;
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
    use bencher_json::{BoundaryUuid, project::boundary::BoundaryLimit};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use crate::{
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_head_version,
            create_measure, create_metric, create_model, create_report, create_report_benchmark,
            create_testbed, create_threshold, create_version, setup_test_db,
        },
    };

    use super::{Detector, PreparedDetection};

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

    use crate::model::project::{
        metric::MetricId,
        threshold::{ThresholdId, model::ModelId},
    };

    /// Set up the full entity chain needed for `PreparedDetection::write` tests.
    /// Returns `(threshold_id, model_id, metric_id)`.
    fn setup_prepared_detection_entities(
        conn: &mut diesel::SqliteConnection,
    ) -> (ThresholdId, ModelId, MetricId) {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let measure = create_measure(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );
        let threshold_id = create_threshold(
            conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );
        let model_id = create_model(
            conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000060",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);
        let report_id = create_report(
            conn,
            "00000000-0000-0000-0000-000000000070",
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
        );
        let benchmark_id = create_benchmark(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000080",
            "bench1",
            "bench1",
        );
        let report_benchmark_id = create_report_benchmark(
            conn,
            "00000000-0000-0000-0000-000000000090",
            report_id,
            0,
            benchmark_id,
        );
        let metric_id = create_metric(
            conn,
            "00000000-0000-0000-0000-0000000000a0",
            report_benchmark_id,
            measure,
            100.0,
        );

        (threshold_id, model_id, metric_id)
    }

    #[test]
    fn prepared_detection_write_inserts_boundary() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: None,
            ignore_benchmark: false,
        };

        detection
            .write(&mut conn, metric_id)
            .expect("Failed to write detection");

        // Assert 1 boundary row exists with correct fields
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .filter(schema::boundary::model_id.eq(model_id))
            .filter(schema::boundary::metric_id.eq(metric_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 0 alert rows exist
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 0);
    }

    #[test]
    fn prepared_detection_write_creates_alert_on_outlier() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: Some(BoundaryLimit::Upper),
            ignore_benchmark: false,
        };

        detection
            .write(&mut conn, metric_id)
            .expect("Failed to write detection");

        // Assert 1 boundary exists
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 1 alert exists
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 1);
    }

    #[test]
    fn prepared_detection_write_skips_alert_when_ignore_benchmark() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: Some(BoundaryLimit::Upper),
            ignore_benchmark: true,
        };

        detection
            .write(&mut conn, metric_id)
            .expect("Failed to write detection");

        // Assert 1 boundary exists
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 0 alerts exist (outlier ignored due to ignore_benchmark)
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 0);
    }
}
