use std::collections::HashMap;

use bencher_json::{BenchmarkNameId, JsonParameters, MeasureNameId};
use serde::Deserialize;

use crate::{
    Adaptable, Settings, results::adapter_metrics::NamedMap,
    results::adapter_results::AdapterResults,
};

/// The BMF v1 wire shape: a benchmark name maps to an array of entries,
/// each carrying a parameter set and its measures.
pub type JsonV1Results = HashMap<BenchmarkNameId, Vec<JsonV1Entry>>;

/// One grid point: what the benchmark ran with, and what it measured.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonV1Entry {
    /// Optional. An entry without it resolves to the benchmark's empty parameter set,
    /// which is exactly what an explicit `{}` resolves to.
    #[serde(default)]
    pub parameters: JsonParameters,
    pub measures: HashMap<MeasureNameId, JsonV1Measure>,
}

/// A measure's named scalars. Every name is equal on the wire.
pub type JsonV1Measure = NamedMap;

/// The BMF v1 leaf of the `json` adapter tree.
///
/// All or nothing: every benchmark in the payload must map to an array, so a
/// payload that mixes v0 objects and v1 arrays fails here and at `json_v0`, and
/// therefore at the `json` node.
pub struct AdapterJsonV1;

impl Adaptable for AdapterJsonV1 {
    fn parse(_input: &str, _settings: Settings) -> Option<AdapterResults> {
        None
    }
}

#[cfg(test)]
pub(crate) mod test_json_v1 {
    use bencher_json::{BenchmarkNameId, JsonParameters, MetricName};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterJsonV1;
    use crate::{
        Adaptable as _, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path},
        results::{
            adapter_metrics::{AdapterMetrics, MAX_METRIC_NAMES},
            adapter_results::{AdapterResults, BmfVersion},
        },
    };

    fn convert_json_v1(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{suffix}.json");
        convert_file_path::<AdapterJsonV1>(&file_path)
    }

    /// The metrics of one benchmark's grid point.
    pub fn grid_point<'r>(
        results: &'r AdapterResults,
        benchmark: &str,
        parameters: &str,
    ) -> &'r AdapterMetrics {
        let benchmark = benchmark
            .parse::<BenchmarkNameId>()
            .expect("Failed to parse benchmark name");
        let parameters = parameters
            .parse::<JsonParameters>()
            .expect("Failed to parse parameters");
        results
            .inner
            .get(&benchmark)
            .expect("Missing benchmark")
            .get(&parameters)
            .expect("Missing parameter set")
    }

    /// One named scalar of one measure.
    pub fn named(metrics: &AdapterMetrics, measure: &str, name: &str) -> Option<OrderedFloat<f64>> {
        metrics
            .inner
            .get(&measure.parse().expect("Failed to parse measure name"))?
            .inner
            .get(
                &name
                    .parse::<MetricName>()
                    .expect("Failed to parse metric name"),
            )
            .copied()
    }

    pub fn names(metrics: &AdapterMetrics, measure: &str) -> Vec<MetricName> {
        metrics
            .inner
            .get(&measure.parse().expect("Failed to parse measure name"))
            .expect("Missing measure")
            .inner
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn adapter_json_v1_latency() {
        let results = convert_json_v1("v1_latency");
        validate_adapter_json_v1_latency(&results);
    }

    pub fn validate_adapter_json_v1_latency(results: &AdapterResults) {
        assert_eq!(results.version, BmfVersion::V1);
        assert_eq!(results.dropped_names, 0);
        assert_eq!(results.inner.len(), 1);

        let benchmark = "tests::benchmark_a".parse::<BenchmarkNameId>().unwrap();
        assert_eq!(results.inner[&benchmark].len(), 2);

        let metrics = grid_point(
            results,
            "tests::benchmark_a",
            r#"{"size_mb": 16, "op": "read"}"#,
        );
        assert_eq!(named(metrics, "latency", "value"), Some(42.0.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(97.0.into()));
        assert_eq!(named(metrics, "latency", "p50"), Some(40.5.into()));

        let metrics = grid_point(
            results,
            "tests::benchmark_a",
            r#"{"size_mb": 32, "op": "read"}"#,
        );
        assert_eq!(named(metrics, "latency", "value"), Some(55.1.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(120.3.into()));
        assert_eq!(named(metrics, "latency", "p50"), Some(51.9.into()));
    }

    /// An entry without `parameters` resolves to the empty parameter set,
    /// which is exactly what an explicit `{}` resolves to: one grid point, not two.
    #[test]
    fn adapter_json_v1_absent_parameters_are_the_empty_set() {
        let results = convert_json_v1("v1_parameters");
        validate_adapter_json_v1_parameters(&results);
    }

    pub fn validate_adapter_json_v1_parameters(results: &AdapterResults) {
        let absent = "tests::absent".parse::<BenchmarkNameId>().unwrap();
        let entries = &results.inner[&absent];
        assert_eq!(entries.len(), 1);
        assert!(
            entries
                .keys()
                .next()
                .expect("Missing parameter set")
                .is_empty()
        );

        // An absent `parameters` and an explicit `{}` merge into one grid point.
        let merged = "tests::merged".parse::<BenchmarkNameId>().unwrap();
        let entries = &results.inner[&merged];
        assert_eq!(entries.len(), 1);
        let metrics = &entries[&JsonParameters::default()];
        assert_eq!(named(metrics, "latency", "value"), Some(1.0.into()));
        assert_eq!(named(metrics, "throughput", "value"), Some(2.0.into()));
    }

    /// Named values are scalars and every name is equal:
    /// a measure may carry only `p99` and never mention `value`.
    #[test]
    fn adapter_json_v1_named_values() {
        let results = convert_json_v1("v1_named");
        validate_adapter_json_v1_named(&results);
    }

    pub fn validate_adapter_json_v1_named(results: &AdapterResults) {
        let metrics = grid_point(results, "tests::percentiles", "{}");
        assert_eq!(names(metrics, "latency"), vec!["p99".parse().unwrap()]);
        assert_eq!(named(metrics, "latency", "p99"), Some(97.0.into()));
        assert_eq!(named(metrics, "latency", "value"), None);
    }

    /// Two entries whose parameters differ only in key order or number spelling
    /// are the same grid point.
    #[test]
    fn adapter_json_v1_canonical_parameters() {
        let results = convert_json_v1("v1_canonical");

        let benchmark = "tests::canonical".parse::<BenchmarkNameId>().unwrap();
        let entries = &results.inner[&benchmark];
        assert_eq!(entries.len(), 1);

        let metrics = grid_point(
            &results,
            "tests::canonical",
            r#"{"op":"read","size_mb":16}"#,
        );
        assert_eq!(named(metrics, "latency", "value"), Some(3.0.into()));
    }

    /// The cap keeps the three conventional names regardless of where they sort,
    /// keeps the rest lexicographically, and reports what it dropped.
    #[test]
    fn adapter_json_v1_named_value_cap() {
        let results = convert_json_v1("v1_cap");
        validate_adapter_json_v1_cap(&results);
    }

    pub fn validate_adapter_json_v1_cap(results: &AdapterResults) {
        let metrics = grid_point(results, "tests::capped", "{}");
        let names = names(metrics, "latency");
        assert_eq!(names.len(), MAX_METRIC_NAMES);
        assert_eq!(
            names,
            vec![
                "a1".parse::<MetricName>().unwrap(),
                "a2".parse().unwrap(),
                "a3".parse().unwrap(),
                "a4".parse().unwrap(),
                "a5".parse().unwrap(),
                "lower_value".parse().unwrap(),
                "upper_value".parse().unwrap(),
                "value".parse().unwrap(),
            ]
        );
        assert_eq!(results.dropped_names, 2);
    }

    /// The survivor set is a property of the payload, not of hash iteration order.
    #[test]
    fn adapter_json_v1_cap_is_deterministic() {
        let expected = convert_json_v1("v1_cap");
        for _ in 0..64u32 {
            assert_eq!(convert_json_v1("v1_cap"), expected);
        }

        // The same names in a different order on the wire keep the same survivors.
        let permuted =
            convert_file_path::<AdapterJsonV1>("./tool_output/json/report_v1_cap_permuted.json");
        let metrics = grid_point(&permuted, "tests::capped", "{}");
        assert_eq!(
            names(metrics, "latency"),
            names(grid_point(&expected, "tests::capped", "{}"), "latency")
        );
        assert_eq!(permuted.dropped_names, 2);
    }

    /// Explicit v1 selection rejects a v0 object payload outright.
    #[test]
    fn adapter_json_v1_rejects_v0() {
        for suffix in ["latency", "dhat", "bmf_mixed"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert!(
                opt_convert_file_path::<AdapterJsonV1>(&file_path, Settings::default()).is_none(),
                "expected {file_path} to be rejected by json_v1"
            );
        }
    }

    /// An empty payload is a well formed v1 payload with nothing in it.
    #[test]
    fn adapter_json_v1_empty() {
        let results = AdapterJsonV1::parse("{}", Settings::default()).unwrap();
        assert!(results.is_empty());
        assert_eq!(results.version, BmfVersion::V1);
    }

    /// Parameter values are JSON scalars only.
    #[test]
    fn adapter_json_v1_rejects_non_scalar_parameters() {
        for parameters in [
            r#"{"a": null}"#,
            r#"{"a": []}"#,
            r#"{"a": {"b": 1}}"#,
            "[]",
            "1",
        ] {
            let input = format!(
                r#"{{"bench": [{{"parameters": {parameters}, "measures": {{"latency": {{"value": 1}}}}}}]}}"#
            );
            assert!(
                AdapterJsonV1::parse(&input, Settings::default()).is_none(),
                "expected {parameters} to be rejected"
            );
        }
    }
}
