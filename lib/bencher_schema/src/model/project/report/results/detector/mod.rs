use std::num::NonZeroU32;

use bencher_boundary::{
    MetricsBoundary,
    correction::{is_corrected, per_test_level},
};
use bencher_json::{Boundary, BoundaryUuid};
use dropshot::HttpError;
use slog::Logger;

use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    error::bad_request_error,
    model::project::{
        benchmark::BenchmarkId, branch::head::HeadId, measure::MeasureId, testbed::TestbedId,
        variant::VariantId,
    },
};

pub mod data;
mod prepared;
pub mod threshold;

pub use prepared::PreparedDetection;

use data::metrics_data;
pub use threshold::{Threshold, ThresholdModel};

#[derive(Debug, Clone)]
pub struct Detector {
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub measure_id: MeasureId,
    pub threshold: Threshold,
    /// The report's family, which corrects a probability boundary at BMF version 1.
    /// `None` tests each line at its boundary, as BMF version 0 does.
    pub family: Option<Family>,
}

impl Detector {
    /// Phase 1: Read historical data and compute the boundary.
    /// Returns a `PreparedDetection` that can be written in Phase 2.
    pub fn prepare_detection(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        variant_id: VariantId,
        metric_value: f64,
        ignore_benchmark: bool,
    ) -> Result<PreparedDetection, HttpError> {
        // Query the historical population/sample data for the variant
        let metrics_data = metrics_data(log, conn, self, benchmark_id, variant_id)?;

        let model = &self.threshold.model;
        let (lower_level, upper_level) = self
            .family
            .map_or((None, None), |family| family.levels(model));

        // Check to see if the metric has a boundary check for the given threshold model.
        let boundary = MetricsBoundary::new(
            log,
            metric_value,
            &metrics_data,
            model.test,
            model.min_sample_size,
            lower_level.or(model.lower_boundary),
            upper_level.or(model.upper_boundary),
        )
        .map_err(bad_request_error)?;
        let MetricsBoundary { limits, outlier } = boundary;

        Ok(PreparedDetection {
            project_id: self.threshold.project_id,
            threshold_id: self.threshold.id,
            model_id: model.id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: limits.baseline,
            lower_limit: limits.lower.map(Into::into),
            lower_level,
            upper_limit: limits.upper.map(Into::into),
            upper_level,
            outlier,
            ignore_benchmark,
        })
    }
}

/// The family of each side of a BMF version 1 report.
///
/// A family is every check a model with a probability boundary on that side makes in the
/// report: each threshold on each line it checks in each iteration, whether or not the line
/// has the history to test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Family {
    pub lower: u32,
    pub upper: u32,
}

impl Family {
    /// Count one check of `model` into the family of each side it guards.
    pub fn count(&mut self, model: &ThresholdModel) {
        if !is_corrected(model.test) {
            return;
        }
        if model.lower_boundary.is_some() {
            self.lower = self.lower.saturating_add(1);
        }
        if model.upper_boundary.is_some() {
            self.upper = self.upper.saturating_add(1);
        }
    }

    /// The level each side of `model` is tested at, or `None` for a side without a boundary
    /// or a model whose boundary is not a probability.
    fn levels(self, model: &ThresholdModel) -> (Option<Boundary>, Option<Boundary>) {
        if !is_corrected(model.test) {
            return (None, None);
        }
        let level = |boundary: Option<Boundary>, family: u32| {
            boundary.map(|boundary| {
                per_test_level(boundary, NonZeroU32::new(family).unwrap_or(NonZeroU32::MIN))
            })
        };
        (
            level(model.lower_boundary, self.lower),
            level(model.upper_boundary, self.upper),
        )
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{MetricName, ParameterFilter, ParameterSet};

    use crate::test_util::{
        create_base_entities, create_branch_with_head, create_measure, create_model,
        create_testbed, create_threshold, create_threshold_with_dimensions, setup_test_db,
    };

    use super::Threshold;

    fn parameters(parameters: &str) -> ParameterSet {
        parameters.parse().expect("Failed to parse parameters")
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
        // that names no metric checks the conventional `value` name of every
        // variant.
        let thresholds =
            Threshold::load(&mut conn, branch.branch_id, testbed).expect("Failed to load");
        let candidates = thresholds.get(&measure).expect("No candidates");
        assert_eq!(candidates.len(), 1);
        let candidate = candidates.first().expect("No candidate");
        assert_eq!(candidate.id, threshold_id);
        assert_eq!(candidate.metric, MetricName::value());
        assert!(candidate.parameters.is_none());
        assert!(candidate.checks(&parameters(r#"{"size": 512}"#)));
        assert!(candidate.checks(&ParameterSet::default()));
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
        let named_id = create_threshold_with_dimensions(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            Some(filter(r#"[{"size": 512}]"#)),
            measure,
            Some(MetricName::try_from("p99".to_owned()).expect("Invalid metric name")),
            "00000000-0000-0000-0000-000000000041",
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
        // The filter names one key, so a variant that pins it matches however many
        // other keys it also pins, and one that pins it differently does not.
        assert!(named_candidate.checks(&parameters(r#"{"size": 512}"#)));
        assert!(named_candidate.checks(&parameters(r#"{"size": 512, "threads": 4}"#)));
        assert!(!named_candidate.checks(&parameters(r#"{"size": 1024}"#)));
        assert!(!named_candidate.checks(&ParameterSet::default()));
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
