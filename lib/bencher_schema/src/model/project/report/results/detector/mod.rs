use bencher_boundary::MetricsBoundary;
use bencher_json::BoundaryUuid;
use dropshot::HttpError;
use slog::Logger;

use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    error::bad_request_error,
    model::project::{
        benchmark::BenchmarkId, branch::head::HeadId, measure::MeasureId, parameter::ParameterId,
        testbed::TestbedId,
    },
};

pub mod data;
mod prepared;
pub mod threshold;

pub use prepared::PreparedDetection;

use data::metrics_data;
pub use threshold::Threshold;

/// One threshold running against one metric row.
///
/// Several thresholds may gate one row, and each is its own detector: its own
/// sample, its own boundary row, and its own alert on a breach.
#[derive(Debug, Clone)]
pub struct Detector {
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub measure_id: MeasureId,
    pub threshold: Threshold,
}

impl Detector {
    /// Phase 1: Read historical data and compute the boundary.
    /// Returns a `PreparedDetection` that can be written in Phase 2.
    pub fn prepare_detection(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        parameter_id: ParameterId,
        metric_value: f64,
        ignore_benchmark: bool,
    ) -> Result<PreparedDetection, HttpError> {
        // Query the historical population/sample data for the grid point
        let metrics_data = metrics_data(log, conn, self, benchmark_id, parameter_id)?;

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
    use bencher_json::{MetricName, ParameterFilter, ParameterSet};

    use crate::test_util::{
        create_base_entities, create_branch_with_head, create_gating_threshold, create_measure,
        create_model, create_testbed, create_threshold, setup_test_db,
    };

    use super::Threshold;

    fn parameters(parameters: &str) -> ParameterSet {
        parameters.parse().expect("Failed to parse parameter set")
    }

    fn filter(filter: &str) -> ParameterFilter {
        filter.parse().expect("Failed to parse parameters filter")
    }

    #[test]
    fn load_returns_nothing_without_threshold() {
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
        create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // No threshold exists => the candidate load is empty
        let thresholds =
            Threshold::load(&mut conn, branch.branch_id, testbed).expect("Failed to load");
        assert!(thresholds.is_empty());
    }

    #[test]
    fn load_returns_the_threshold_with_a_model() {
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

        // Threshold + model exist => the candidate load names it, and a threshold
        // that names no metric gates the conventional `value` name of every grid
        // point.
        let thresholds =
            Threshold::load(&mut conn, branch.branch_id, testbed).expect("Failed to load");
        let candidates = thresholds.get(&measure).expect("No candidates");
        assert_eq!(candidates.len(), 1);
        let candidate = candidates.first().expect("No candidate");
        assert_eq!(candidate.id, threshold_id);
        assert_eq!(candidate.metric, MetricName::value());
        assert!(candidate.parameters.is_none());
        assert!(candidate.gates(&parameters(r#"{"size": 512}"#)));
        assert!(candidate.gates(&ParameterSet::default()));
    }

    #[test]
    fn load_returns_every_threshold_of_one_measure() {
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
        let bare_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(
            &mut conn,
            bare_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );
        let named_id = create_gating_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000041",
            Some(MetricName::try_from("p99".to_owned()).expect("Invalid metric name")),
            Some(filter(r#"[{"size": 512}]"#)),
        );
        create_model(
            &mut conn,
            named_id,
            "00000000-0000-0000-0000-000000000051",
            0,
        );

        let thresholds =
            Threshold::load(&mut conn, branch.branch_id, testbed).expect("Failed to load");
        let candidates = thresholds.get(&measure).expect("No candidates");
        assert_eq!(candidates.len(), 2);

        let bare_candidate = candidates.first().expect("No bare candidate");
        assert_eq!(bare_candidate.id, bare_id);
        assert_eq!(bare_candidate.metric, MetricName::value());

        let named_candidate = candidates.get(1).expect("No named candidate");
        assert_eq!(named_candidate.id, named_id);
        assert_eq!(named_candidate.metric.as_ref(), "p99");
        // The filter names one key, so a grid point that pins it matches however many
        // other keys it also pins, and one that pins it differently does not.
        assert!(named_candidate.gates(&parameters(r#"{"size": 512}"#)));
        assert!(named_candidate.gates(&parameters(r#"{"size": 512, "threads": 4}"#)));
        assert!(!named_candidate.gates(&parameters(r#"{"size": 1024}"#)));
        assert!(!named_candidate.gates(&ParameterSet::default()));
    }

    #[test]
    fn load_skips_a_threshold_without_a_model() {
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
        create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // A threshold without a model runs no test, so it is not a candidate.
        let thresholds =
            Threshold::load(&mut conn, branch.branch_id, testbed).expect("Failed to load");
        assert!(thresholds.is_empty());
    }
}
