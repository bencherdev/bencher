use std::collections::HashMap;

use bencher_client::types::JsonReportThresholds;
use bencher_json::{Boundary, MeasureNameId, SampleSize, Window};

use crate::{
    ThresholdError,
    bencher::sub::project::threshold::model::Model,
    parser::{
        ElidedOption,
        project::{
            report::CliReportThresholds,
            threshold::{CliModel, CliModelTest},
        },
    },
};

#[derive(Debug, Clone)]
pub struct Thresholds {
    models: Option<HashMap<String, bencher_client::types::Model>>,
    reset: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum ThresholdsError {
    #[error(
        "The {0} Measure Threshold is missing its model test. Use the `--threshold-test` option to set the test."
    )]
    MissingTest(MeasureNameId),
    #[error("Failed to validate the model for the {measure} Measure Threshold: {err}")]
    BadModel {
        measure: MeasureNameId,
        err: ThresholdError,
    },
    #[error("There are more model tests than Measures: {0:?}")]
    ExtraTests(Vec<CliModelTest>),
    #[error("There are more minimum sample sizes than model tests")]
    ExtraMinSampleSizes(Vec<ElidedOption<SampleSize>>),
    #[error("There are more maximum sample sizes than model tests")]
    ExtraMaxSampleSizes(Vec<ElidedOption<SampleSize>>),
    #[error("There are more windows than model tests")]
    ExtraWindows(Vec<ElidedOption<Window>>),
    #[error("There are more lower boundaries than model tests")]
    ExtraLowerBoundaries(Vec<ElidedOption<Boundary>>),
    #[error("There are more upper boundaries than model tests")]
    ExtraUpperBoundaries(Vec<ElidedOption<Boundary>>),
}

impl TryFrom<CliReportThresholds> for Thresholds {
    type Error = ThresholdsError;

    fn try_from(thresholds: CliReportThresholds) -> Result<Self, Self::Error> {
        let CliReportThresholds {
            threshold_measure,
            threshold_test,
            threshold_min_sample_size,
            threshold_max_sample_size,
            threshold_window,
            threshold_lower_boundary,
            threshold_upper_boundary,
            thresholds_reset,
        } = thresholds;

        let mut models_map = HashMap::with_capacity(threshold_measure.len());

        let mut tests = threshold_test.into_iter();
        let mut min_sample_sizes = threshold_min_sample_size.into_iter();
        let mut max_sample_sizes = threshold_max_sample_size.into_iter();
        let mut windows = threshold_window.into_iter();
        let mut lower_boundaries = threshold_lower_boundary.into_iter();
        let mut upper_boundaries = threshold_upper_boundary.into_iter();
        for measure in threshold_measure {
            let test = tests
                .next()
                .ok_or(ThresholdsError::MissingTest(measure.clone()))?;
            let min_sample_size = min_sample_sizes.next();
            let max_sample_size = max_sample_sizes.next();
            let window = windows.next();
            let lower_boundary = lower_boundaries.next();
            let upper_boundary = upper_boundaries.next();

            let cli_model = CliModel {
                test,
                min_sample_size: min_sample_size.and_then(Into::into),
                max_sample_size: max_sample_size.and_then(Into::into),
                window: window.and_then(Into::into),
                lower_boundary: lower_boundary.and_then(Into::into),
                upper_boundary: upper_boundary.and_then(Into::into),
            };
            let model = Model::try_from(cli_model).map_err(|err| ThresholdsError::BadModel {
                measure: measure.clone(),
                err,
            })?;

            models_map.insert(measure.to_string(), model.into());
        }

        let remaining_tests = tests.collect::<Vec<_>>();
        if !remaining_tests.is_empty() {
            return Err(ThresholdsError::ExtraTests(remaining_tests));
        }
        let remaining_min_sample_sizes = min_sample_sizes.collect::<Vec<_>>();
        if !remaining_min_sample_sizes.is_empty() {
            return Err(ThresholdsError::ExtraMinSampleSizes(
                remaining_min_sample_sizes,
            ));
        }
        let remaining_max_sample_sizes = max_sample_sizes.collect::<Vec<_>>();
        if !remaining_max_sample_sizes.is_empty() {
            return Err(ThresholdsError::ExtraMaxSampleSizes(
                remaining_max_sample_sizes,
            ));
        }
        let remaining_windows = windows.collect::<Vec<_>>();
        if !remaining_windows.is_empty() {
            return Err(ThresholdsError::ExtraWindows(remaining_windows));
        }
        let remaining_lower_boundaries = lower_boundaries.collect::<Vec<_>>();
        if !remaining_lower_boundaries.is_empty() {
            return Err(ThresholdsError::ExtraLowerBoundaries(
                remaining_lower_boundaries,
            ));
        }
        let remaining_upper_boundaries = upper_boundaries.collect::<Vec<_>>();
        if !remaining_upper_boundaries.is_empty() {
            return Err(ThresholdsError::ExtraUpperBoundaries(
                remaining_upper_boundaries,
            ));
        }

        Ok(Self {
            // Do not short circuit early if there are no measures
            // because we need to still check if there are any dangling options
            models: if models_map.is_empty() {
                None
            } else {
                Some(models_map)
            },
            reset: thresholds_reset,
        })
    }
}

impl From<Thresholds> for Option<JsonReportThresholds> {
    fn from(thresholds: Thresholds) -> Self {
        let Thresholds { models, reset } = thresholds;
        if models.is_none() && !reset {
            None
        } else {
            Some(JsonReportThresholds {
                models,
                reset: reset.then_some(reset),
            })
        }
    }
}
