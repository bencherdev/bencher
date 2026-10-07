use bencher_client::types::{
    JsonReportThresholdEntry, JsonReportThresholdModels, JsonReportThresholds,
};
use bencher_json::{
    Boundary, MeasureNameId, MetricName, ParameterFilter, ParameterSet, SampleSize, Window,
};

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
    models: Option<JsonReportThresholdModels>,
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
    #[error(
        "The {0} Measure Threshold is missing its metric. Once any Threshold has a metric or parameters, every Threshold needs one. Use the `--threshold-metric` option to set the metric."
    )]
    MissingMetric(MeasureNameId),
    #[error("Failed to send the parameters of the {measure} Measure Threshold: {err}")]
    BadParameters {
        measure: MeasureNameId,
        err: serde_json::Error,
    },
    #[error("There are more metrics than Measures")]
    ExtraMetrics(Vec<ElidedOption<MetricName>>),
    #[error("There are more parameters than Measures")]
    ExtraParameters(Vec<ElidedOption<ParameterSet>>),
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
            threshold_metric,
            threshold_parameters,
            threshold_test,
            threshold_min_sample_size,
            threshold_max_sample_size,
            threshold_window,
            threshold_lower_boundary,
            threshold_upper_boundary,
            thresholds_reset,
        } = thresholds;

        let mut declared = Vec::with_capacity(threshold_measure.len());

        let mut metrics = threshold_metric.into_iter();
        let mut parameters = threshold_parameters.into_iter();
        let mut tests = threshold_test.into_iter();
        let mut min_sample_sizes = threshold_min_sample_size.into_iter();
        let mut max_sample_sizes = threshold_max_sample_size.into_iter();
        let mut windows = threshold_window.into_iter();
        let mut lower_boundaries = threshold_lower_boundary.into_iter();
        let mut upper_boundaries = threshold_upper_boundary.into_iter();
        for measure in threshold_measure {
            let metric = metrics.next().and_then(Into::into);
            let parameter_set = parameters.next().and_then(Into::into);
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

            declared.push(Declared {
                measure,
                metric,
                parameters: parameter_set,
                model,
            });
        }

        let remaining_metrics = metrics.collect::<Vec<_>>();
        if !remaining_metrics.is_empty() {
            return Err(ThresholdsError::ExtraMetrics(remaining_metrics));
        }
        let remaining_parameters = parameters.collect::<Vec<_>>();
        if !remaining_parameters.is_empty() {
            return Err(ThresholdsError::ExtraParameters(remaining_parameters));
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
            models: into_models(declared)?,
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

/// The BMF version 1 list once any threshold gives a metric or parameters, else the version 0 map.
fn into_models(
    declared: Vec<Declared>,
) -> Result<Option<JsonReportThresholdModels>, ThresholdsError> {
    if declared.is_empty() {
        return Ok(None);
    }
    let models = if declared
        .iter()
        .any(|threshold| threshold.metric.is_some() || threshold.parameters.is_some())
    {
        JsonReportThresholdModels::List(
            declared
                .into_iter()
                .map(Declared::into_entry)
                .collect::<Result<_, _>>()?,
        )
    } else {
        JsonReportThresholdModels::Map(
            declared
                .into_iter()
                .map(|threshold| (threshold.measure.to_string(), threshold.model.into()))
                .collect(),
        )
    };
    Ok(Some(models))
}

/// One threshold the flags declare.
struct Declared {
    measure: MeasureNameId,
    metric: Option<MetricName>,
    parameters: Option<ParameterSet>,
    model: Model,
}

impl Declared {
    fn into_entry(self) -> Result<JsonReportThresholdEntry, ThresholdsError> {
        let Self {
            measure,
            metric,
            parameters,
            model,
        } = self;
        let metric = metric.ok_or_else(|| ThresholdsError::MissingMetric(measure.clone()))?;
        // The client spells a filter as plain JSON, so the one set travels through it.
        let parameters = parameters
            .map(|set| {
                serde_json::to_value(ParameterFilter::new(vec![set]))
                    .and_then(serde_json::from_value)
            })
            .transpose()
            .map_err(|err| ThresholdsError::BadParameters {
                measure: measure.clone(),
                err,
            })?;
        Ok(JsonReportThresholdEntry {
            measure: measure.into(),
            metric: metric.into(),
            model: model.into(),
            parameters,
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::{JsonReportThresholds, Thresholds, ThresholdsError};
    use crate::parser::project::report::CliReportThresholds;

    // Every threshold gives every option, with an underscore for the one it goes without.
    const LATENCY: [&str; 8] = [
        "--threshold-measure",
        "latency",
        "--threshold-test",
        "t_test",
        "--threshold-lower-boundary",
        "_",
        "--threshold-upper-boundary",
        "0.99",
    ];
    const THROUGHPUT: [&str; 8] = [
        "--threshold-measure",
        "throughput",
        "--threshold-test",
        "t_test",
        "--threshold-lower-boundary",
        "0.99",
        "--threshold-upper-boundary",
        "_",
    ];

    // Without a metric the flags send the BMF version 0 map, as they always have.
    #[test]
    fn thresholds_without_a_metric_are_a_map() {
        let flags = [LATENCY.as_slice(), THROUGHPUT.as_slice()].concat();
        assert_eq!(
            thresholds(&flags).unwrap(),
            serde_json::json!({
                "models": {
                    "latency": model("upper_boundary"),
                    "throughput": model("lower_boundary"),
                },
            })
        );
    }

    // A metric sends the BMF version 1 list, each entry with its own metric and its parameters as
    // a filter of one set, an elided parameters option leaves its entry checking every variant, and
    // the reset flag still travels beside the list.
    #[test]
    fn thresholds_with_a_metric_are_a_list() {
        let flags = [
            LATENCY.as_slice(),
            &[
                "--threshold-metric",
                "p99",
                "--threshold-parameters",
                r#"{"size": 1, "mode": "fast"}"#,
            ],
            THROUGHPUT.as_slice(),
            &[
                "--threshold-metric",
                "value",
                "--threshold-parameters",
                "_",
                "--thresholds-reset",
            ],
        ]
        .concat();
        assert_eq!(
            thresholds(&flags).unwrap(),
            serde_json::json!({
                "models": [
                    {
                        "measure": "latency",
                        "metric": "p99",
                        "parameters": [{ "mode": "fast", "size": 1.0 }],
                        "model": model("upper_boundary"),
                    },
                    {
                        "measure": "throughput",
                        "metric": "value",
                        "model": model("lower_boundary"),
                    },
                ],
                "reset": true,
            })
        );
    }

    // Once any threshold gives a metric or parameters, every threshold needs a metric, and the
    // refusal names the measure that has none.
    #[test]
    fn thresholds_in_a_list_each_need_a_metric() {
        for (latency, throughput, missing) in [
            (
                ["--threshold-metric", "p99"],
                ["--threshold-metric", "_"],
                "throughput",
            ),
            (
                ["--threshold-metric", "_"],
                ["--threshold-parameters", r#"{"size": 1}"#],
                "latency",
            ),
        ] {
            let flags = [
                LATENCY.as_slice(),
                &latency,
                THROUGHPUT.as_slice(),
                &throughput,
            ]
            .concat();
            let err = thresholds(&flags).unwrap_err();
            assert!(
                matches!(&err, ThresholdsError::MissingMetric(measure) if measure.to_string() == missing),
                "{err}"
            );
        }
    }

    // A metric or parameters option with no measure left to pair with is refused.
    #[test]
    fn thresholds_refuse_a_dangling_metric_or_parameters() {
        let metrics = [
            LATENCY.as_slice(),
            &["--threshold-metric", "p99", "--threshold-metric", "value"],
        ]
        .concat();
        assert!(matches!(
            thresholds(&metrics),
            Err(ThresholdsError::ExtraMetrics(_))
        ));
        let parameters = [
            LATENCY.as_slice(),
            &[
                "--threshold-metric",
                "p99",
                "--threshold-parameters",
                "_",
                "--threshold-parameters",
                "_",
            ],
        ]
        .concat();
        assert!(matches!(
            thresholds(&parameters),
            Err(ThresholdsError::ExtraParameters(_))
        ));
    }

    #[derive(clap::Parser)]
    struct Cli {
        #[clap(flatten)]
        thresholds: CliReportThresholds,
    }

    /// The thresholds the flags send, as the request body carries them.
    fn thresholds(flags: &[&str]) -> Result<serde_json::Value, ThresholdsError> {
        Thresholds::try_from(parse(flags)).map(body)
    }

    fn parse(flags: &[&str]) -> CliReportThresholds {
        Cli::try_parse_from(std::iter::once("bencher").chain(flags.iter().copied()))
            .unwrap()
            .thresholds
    }

    fn body(thresholds: Thresholds) -> serde_json::Value {
        serde_json::to_value(Option::<JsonReportThresholds>::from(thresholds)).unwrap()
    }

    fn model(boundary: &str) -> serde_json::Value {
        serde_json::json!({ "test": "t_test", boundary: 0.99 })
    }
}
