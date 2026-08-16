use std::collections::HashMap;

use bencher_json::{BenchmarkNameId, JsonNewMetric, JsonParameters, MeasureNameId};

use crate::{
    Adaptable, Settings,
    results::adapter_results::{AdapterResults, ResultsMap},
};

/// The BMF v0 wire shape: a benchmark name maps to its measures,
/// and each measure to a metric triple.
pub type JsonV0Results = HashMap<BenchmarkNameId, JsonV0Measures>;

pub type JsonV0Measures = HashMap<MeasureNameId, JsonNewMetric>;

/// The BMF v0 leaf of the `json` adapter tree.
///
/// All or nothing: every benchmark in the payload must map to an object, so a
/// payload that mixes v0 objects and v1 arrays fails here and at `json_v1`, and
/// therefore at the `json` node.
pub struct AdapterJsonV0;

impl Adaptable for AdapterJsonV0 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let results: JsonV0Results = serde_json::from_str(input).ok()?;
        Some(from_wire(results))
    }
}

/// Every v0 benchmark rides the empty parameter set, and every metric triple
/// becomes exactly the conventional `value`, `lower_value`, and `upper_value` names.
fn from_wire(results: JsonV0Results) -> AdapterResults {
    results
        .into_iter()
        .map(|(benchmark, measures)| {
            let metrics = measures
                .into_iter()
                .map(|(measure, metric)| (measure, metric.into()))
                .collect::<HashMap<_, _>>();
            (
                benchmark,
                std::iter::once((JsonParameters::default(), metrics.into())).collect(),
            )
        })
        .collect::<ResultsMap>()
        .into()
}

#[cfg(test)]
pub(crate) mod test_json_v0 {
    use bencher_json::{BenchmarkNameId, MetricName};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterJsonV0;
    use crate::{
        Adaptable as _, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        results::adapter_results::{AdapterResults, BmfVersion},
    };

    fn convert_json_v0(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{suffix}.json");
        convert_file_path::<AdapterJsonV0>(&file_path)
    }

    #[test]
    fn adapter_json_v0_latency() {
        let results = convert_json_v0("latency");
        validate_adapter_json_latency(&results);
    }

    pub fn validate_adapter_json_latency(results: &AdapterResults) {
        assert_eq!(results.version, BmfVersion::V0);
        assert_eq!(results.dropped_names, 0);
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_latency(metrics, 3247.0, Some(1044.0), Some(1044.0));

        let metrics = results.get("tests::benchmark_b").unwrap();
        validate_latency(metrics, 3443.0, Some(2275.0), Some(2275.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_latency(metrics, 3361.0, Some(1093.0), Some(1093.0));
    }

    #[test]
    fn adapter_json_v0_dhat() {
        let results = convert_json_v0("dhat");
        validate_adapter_json_dhat(&results);
    }

    pub fn validate_adapter_json_dhat(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 1);
        let metrics = results.get("bench_play_game").unwrap();
        assert_eq!(metrics.inner.len(), 6);
        for (key, value) in [
            ("Final Blocks", 0.0),
            ("Final Bytes", 0.0),
            ("Max Blocks", 1.0),
            ("Max Bytes", 9.0),
            ("Total Blocks", 100.0),
            ("Total Bytes", 662.0),
        ] {
            let metric = metrics.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(value));
            assert_eq!(metric.lower_value, None);
            assert_eq!(metric.upper_value, None);
        }
    }

    #[test]
    fn adapter_json_v0_bmf_mixed() {
        let results = convert_json_v0("bmf_mixed");
        validate_adapter_json_bmf_mixed(&results);
    }

    pub fn validate_adapter_json_bmf_mixed(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 3);

        let uuid =
            BenchmarkNameId::new_uuid("31aba8a9-977a-47d1-9fb6-e6b94b428471".parse().unwrap());
        let uuid_metrics = results.entry(&uuid).unwrap();
        validate_latency(uuid_metrics, 3247.0, Some(1044.0), Some(1044.0));

        let slug = BenchmarkNameId::new_slug("benchmark-b".parse().unwrap());
        let slug_metrics = results.entry(&slug).unwrap();
        validate_latency(slug_metrics, 3443.0, Some(2275.0), Some(2275.0));

        let name = BenchmarkNameId::new_name("tests::benchmark_c".parse().unwrap());
        let name_metrics = results.entry(&name).unwrap();
        validate_latency(name_metrics, 3361.0, Some(1093.0), Some(1093.0));
    }

    /// A metric triple becomes exactly the three conventional names,
    /// and a metric without bounds becomes exactly one.
    #[test]
    fn adapter_json_v0_conventional_names() {
        let results = convert_json_v0("latency");
        let metrics = results.get("tests::benchmark_a").unwrap();
        let metric = metrics
            .inner
            .get(&"latency".parse().unwrap())
            .expect("Missing latency measure");
        assert_eq!(
            metric.inner.keys().cloned().collect::<Vec<MetricName>>(),
            vec![
                MetricName::lower_value(),
                MetricName::upper_value(),
                MetricName::value()
            ]
        );

        let results = convert_json_v0("dhat");
        let metrics = results.get("bench_play_game").unwrap();
        let metric = metrics
            .inner
            .get(&"Max Bytes".parse().unwrap())
            .expect("Missing Max Bytes measure");
        assert_eq!(
            metric.inner.keys().cloned().collect::<Vec<MetricName>>(),
            vec![MetricName::value()]
        );
    }

    /// Explicit v0 selection rejects a v1 array payload outright.
    #[test]
    fn adapter_json_v0_rejects_v1() {
        for suffix in ["v1_latency", "v1_parameters", "v1_named"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert!(
                opt_convert_file_path::<AdapterJsonV0>(&file_path, Settings::default()).is_none(),
                "expected {file_path} to be rejected by json_v0"
            );
        }
    }

    /// An empty payload is a well formed v0 payload with nothing in it.
    #[test]
    fn adapter_json_v0_empty() {
        let results = AdapterJsonV0::parse("{}", Settings::default()).unwrap();
        assert!(results.is_empty());
        assert_eq!(results.version, BmfVersion::V0);
    }
}
