use bencher_client::types::{
    JsonReportThresholdEntry, JsonReportThresholdModels, JsonReportThresholds,
};
use bencher_json::{
    Boundary, Confirmations, MeasureNameId, MetricName, ModelSlot, ParameterFilter, ParameterSet,
    SampleSize, Significance, Window,
};

use crate::{
    ThresholdError,
    bencher::sub::project::threshold::model::Model,
    parser::{
        ElidedOption,
        project::{report::CliReportThresholds, threshold::CliModelTest},
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
    #[error("There are more significances than model tests")]
    ExtraSignificances(Vec<ElidedOption<Significance>>),
    #[error("There are more confirmations than model tests")]
    ExtraConfirmations(Vec<ElidedOption<Confirmations>>),
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
            threshold_significance,
            threshold_confirmations,
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
        let mut significances = threshold_significance.into_iter();
        let mut confirmations = threshold_confirmations.into_iter();
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
            let significance = significances.next();
            let confirmation = confirmations.next();

            let test = bencher_json::ModelTest::from(test);
            let slot = test.slot();
            let json_model = bencher_json::Model {
                test,
                min_sample_size: min_sample_size.and_then(Into::into),
                max_sample_size: max_sample_size.and_then(Into::into),
                window: window.and_then(Into::into),
                lower_boundary: lower_boundary.and_then(Into::into),
                upper_boundary: upper_boundary.and_then(Into::into),
                significance: significance.and_then(Into::into),
                confirmations: confirmation.and_then(Into::into),
            };
            let model = Model::try_from(json_model).map_err(|err| ThresholdsError::BadModel {
                measure: measure.clone(),
                err,
            })?;

            declared.push(Declared {
                measure,
                metric,
                parameters: parameter_set,
                slot,
                model,
            });
        }

        leftover(metrics, ThresholdsError::ExtraMetrics)?;
        leftover(parameters, ThresholdsError::ExtraParameters)?;
        leftover(tests, ThresholdsError::ExtraTests)?;
        leftover(min_sample_sizes, ThresholdsError::ExtraMinSampleSizes)?;
        leftover(max_sample_sizes, ThresholdsError::ExtraMaxSampleSizes)?;
        leftover(windows, ThresholdsError::ExtraWindows)?;
        leftover(lower_boundaries, ThresholdsError::ExtraLowerBoundaries)?;
        leftover(upper_boundaries, ThresholdsError::ExtraUpperBoundaries)?;
        leftover(significances, ThresholdsError::ExtraSignificances)?;
        leftover(confirmations, ThresholdsError::ExtraConfirmations)?;

        Ok(Self {
            // Do not short circuit early if there are no measures
            // because we need to still check if there are any dangling options
            models: into_models(declared)?,
            reset: thresholds_reset,
        })
    }
}

/// Refuse the values of an option that no measure is left to take.
fn leftover<T>(
    values: impl Iterator<Item = T>,
    extra: impl FnOnce(Vec<T>) -> ThresholdsError,
) -> Result<(), ThresholdsError> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        Ok(())
    } else {
        Err(extra(values))
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

/// The BMF version 1 list once any threshold gives a metric or parameters, or a model that belongs
/// in the changepoint slot, else the version 0 map, whose models all belong in the metric slot.
fn into_models(
    declared: Vec<Declared>,
) -> Result<Option<JsonReportThresholdModels>, ThresholdsError> {
    if declared.is_empty() {
        return Ok(None);
    }
    let models = if declared.iter().any(|threshold| {
        threshold.metric.is_some()
            || threshold.parameters.is_some()
            || threshold.slot == ModelSlot::ChangepointModel
    }) {
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
    /// The server routes a model to its slot by its test, so only the shape of the body needs it.
    slot: ModelSlot,
    model: Model,
}

impl Declared {
    fn into_entry(self) -> Result<JsonReportThresholdEntry, ThresholdsError> {
        let Self {
            measure,
            metric,
            parameters,
            model,
            ..
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
    const CHANGEPOINT: [&str; 10] = [
        "--threshold-measure",
        "latency",
        "--threshold-test",
        "changepoint",
        "--threshold-lower-boundary",
        "_",
        "--threshold-upper-boundary",
        "0.05",
        "--threshold-max-sample-size",
        "50",
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

    // A group with the changepoint test checks a line, so it sends the BMF version 1 list, with its
    // significance, and a metric group and a changepoint group for one key are two entries.
    #[test]
    fn a_changepoint_group_goes_in_the_list() {
        let flags = [
            LATENCY.as_slice(),
            &[
                "--threshold-metric",
                "p99",
                "--threshold-max-sample-size",
                "_",
                "--threshold-significance",
                "_",
            ],
            CHANGEPOINT.as_slice(),
            &[
                "--threshold-metric",
                "p99",
                "--threshold-significance",
                "0.001",
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
                        "model": model("upper_boundary"),
                    },
                    {
                        "measure": "latency",
                        "metric": "p99",
                        "model": {
                            "test": "changepoint",
                            "upper_boundary": 0.05,
                            "max_sample_size": 50,
                            "significance": 0.001,
                        },
                    },
                ],
            })
        );
    }

    // The list needs a metric for every entry, so a changepoint group needs one too.
    #[test]
    fn a_changepoint_group_needs_a_metric() {
        let flags = [
            CHANGEPOINT.as_slice(),
            &["--threshold-significance", "0.001"],
        ]
        .concat();
        let err = thresholds(&flags).unwrap_err();
        assert!(
            matches!(&err, ThresholdsError::MissingMetric(measure) if measure.to_string() == "latency"),
            "{err}"
        );
    }

    // A changepoint group sends the count of confirming scans it names.
    #[test]
    fn a_changepoint_group_sends_its_confirmations() {
        let flags = [
            CHANGEPOINT.as_slice(),
            &[
                "--threshold-metric",
                "p99",
                "--threshold-significance",
                "0.001",
                "--threshold-confirmations",
                "3",
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
                        "model": {
                            "test": "changepoint",
                            "upper_boundary": 0.05,
                            "max_sample_size": 50,
                            "significance": 0.001,
                            "confirmations": 3,
                        },
                    },
                ],
            })
        );
    }

    // A count of confirming scans with no group left to pair with is refused, and so is one on a
    // metric test, and one out of its range is refused as the flag is read.
    #[test]
    fn thresholds_refuse_a_dangling_or_misplaced_confirmations() {
        let dangling = [
            LATENCY.as_slice(),
            &[
                "--threshold-confirmations",
                "_",
                "--threshold-confirmations",
                "5",
            ],
        ]
        .concat();
        assert!(matches!(
            thresholds(&dangling),
            Err(ThresholdsError::ExtraConfirmations(_))
        ));
        let misplaced = [LATENCY.as_slice(), &["--threshold-confirmations", "5"]].concat();
        let err = thresholds(&misplaced).unwrap_err();
        assert!(
            matches!(&err, ThresholdsError::BadModel { .. })
                && err.to_string().contains("`confirmations`"),
            "{err}"
        );
        for out_of_range in ["0", "21"] {
            let flags = [
                LATENCY.as_slice(),
                &["--threshold-confirmations", out_of_range],
            ]
            .concat();
            let err = Cli::try_parse_from(std::iter::once("bencher").chain(flags))
                .err()
                .expect("the flag is refused");
            assert!(err.to_string().contains("`confirmations`"), "{err}");
        }
    }

    // A significance with no group left to pair with is refused, and so is one on a metric test.
    #[test]
    fn thresholds_refuse_a_dangling_or_misplaced_significance() {
        let dangling = [
            LATENCY.as_slice(),
            &[
                "--threshold-significance",
                "_",
                "--threshold-significance",
                "0.001",
            ],
        ]
        .concat();
        assert!(matches!(
            thresholds(&dangling),
            Err(ThresholdsError::ExtraSignificances(_))
        ));
        let misplaced = [LATENCY.as_slice(), &["--threshold-significance", "0.001"]].concat();
        let err = thresholds(&misplaced).unwrap_err();
        assert!(
            matches!(&err, ThresholdsError::BadModel { .. })
                && err.to_string().contains("`significance`"),
            "{err}"
        );
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
