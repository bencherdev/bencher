use std::{collections::HashMap, iter};

use bencher_valid::{BenchmarkName, BmfVersion, MetricName};
use ordered_float::OrderedFloat;
use serde::Serialize;

use super::{JsonMetricsMap, JsonNewMetric, JsonResultsMap, MetricResults};
use crate::MeasureNameId;

/// Benchmark results as BMF, at either version.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum JsonNewResults {
    V0(JsonResultsMap),
    V1(HashMap<BenchmarkName, Vec<JsonNewVariant>>),
}

impl JsonNewResults {
    /// The results at `bmf_version`. At version 1 each benchmark is one variant with no
    /// parameters, and each scalar is a metric by its name.
    pub fn new(results: MetricResults, bmf_version: BmfVersion) -> Self {
        let results = JsonNewMetric::results(results);
        if bmf_version == BmfVersion::V1 {
            Self::V1(
                results
                    .into_iter()
                    .map(|(benchmark_name, metrics)| {
                        (benchmark_name, vec![JsonNewVariant::from(metrics)])
                    })
                    .collect(),
            )
        } else {
            Self::V0(results)
        }
    }
}

/// One BMF version 1 entry of a benchmark: its measures, each with its named metrics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonNewVariant {
    pub measures: HashMap<MeasureNameId, HashMap<MetricName, OrderedFloat<f64>>>,
}

impl From<JsonMetricsMap> for JsonNewVariant {
    fn from(metrics: JsonMetricsMap) -> Self {
        Self {
            measures: metrics
                .into_iter()
                .map(|(measure, metric)| {
                    let JsonNewMetric {
                        value,
                        lower_value,
                        upper_value,
                    } = metric;
                    let named = iter::once((MetricName::value(), value))
                        .chain(lower_value.map(|lower| (MetricName::lower_value(), lower)))
                        .chain(upper_value.map(|upper| (MetricName::upper_value(), upper)))
                        .collect();
                    (measure, named)
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_valid::BmfVersion;
    use pretty_assertions::assert_eq;

    use super::JsonNewResults;
    use crate::{JsonNewMetric, project::metric::MetricResults};

    // Kills: version 0 results that change shape, or that take the version 1 shape.
    #[test]
    fn version_0_is_the_metric_triple_map() {
        let results = vec![(
            "./build.sh".parse().unwrap(),
            vec![("build-time".parse().unwrap(), metric(1.5))],
        )];
        assert_eq!(
            json(results, BmfVersion::V0),
            serde_json::json!({
                "./build.sh": {
                    "build-time": { "value": 1.5, "lower_value": null, "upper_value": null }
                }
            })
        );
    }

    // Kills: version 1 results in the version 0 shape, with parameters, or with null bounds.
    #[test]
    fn version_1_is_one_value_per_measure_with_no_parameters() {
        let results = vec![(
            "./build.sh".parse().unwrap(),
            vec![
                ("build-time".parse().unwrap(), metric(1.5)),
                ("file-size".parse().unwrap(), metric(64.0)),
            ],
        )];
        assert_eq!(
            json(results, BmfVersion::V1),
            serde_json::json!({
                "./build.sh": [{
                    "measures": {
                        "build-time": { "value": 1.5 },
                        "file-size": { "value": 64.0 },
                    }
                }]
            })
        );
    }

    // Kills: version 1 results that drop the bounds of a metric triple.
    #[test]
    fn version_1_names_each_bound() {
        let results = vec![(
            "bench".parse().unwrap(),
            vec![(
                "latency".parse().unwrap(),
                JsonNewMetric {
                    value: 2.0.into(),
                    lower_value: Some(1.0.into()),
                    upper_value: Some(3.0.into()),
                },
            )],
        )];
        assert_eq!(
            json(results, BmfVersion::V1),
            serde_json::json!({
                "bench": [{
                    "measures": {
                        "latency": { "value": 2.0, "lower_value": 1.0, "upper_value": 3.0 }
                    }
                }]
            })
        );
    }

    // Kills: version 1 results that keep both of two results for one benchmark name, where
    // version 0 keeps only the later.
    #[test]
    fn version_1_keeps_the_later_of_one_name_as_version_0_does() {
        let results = || {
            vec![
                (
                    "app".parse().unwrap(),
                    vec![("file-size".parse().unwrap(), metric(64.0))],
                ),
                (
                    "app".parse().unwrap(),
                    vec![("build-time".parse().unwrap(), metric(1.5))],
                ),
            ]
        };
        assert_eq!(
            json(results(), BmfVersion::V1),
            serde_json::json!({
                "app": [{ "measures": { "build-time": { "value": 1.5 } } }]
            })
        );
        assert_eq!(
            json(results(), BmfVersion::V0),
            serde_json::json!({
                "app": {
                    "build-time": { "value": 1.5, "lower_value": null, "upper_value": null }
                }
            })
        );
    }

    fn metric(value: f64) -> JsonNewMetric {
        JsonNewMetric {
            value: value.into(),
            ..Default::default()
        }
    }

    fn json(results: MetricResults, bmf_version: BmfVersion) -> serde_json::Value {
        serde_json::to_value(JsonNewResults::new(results, bmf_version)).unwrap()
    }
}
