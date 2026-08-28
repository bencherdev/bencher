use std::collections::HashMap;

use bencher_json::{BenchmarkNameId, MeasureNameId, ParameterSet};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    results::{
        adapter_metrics::{AdapterMetrics, NamedMap},
        adapter_results::{AdapterResults, BenchmarkEntries, BmfVersion, ResultsMap},
    },
};

/// The BMF v1 wire shape: a benchmark name maps to an array of entries,
/// each carrying a parameter set and its measures.
pub type JsonV1Results = HashMap<BenchmarkNameId, Vec<JsonV1Entry>>;

/// One variant: what the benchmark ran with, and what it measured.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonV1Entry {
    /// Optional. An entry without it resolves to the benchmark's empty parameter set,
    /// which is exactly what an explicit `{}` resolves to.
    #[serde(default)]
    pub parameters: ParameterSet,
    pub measures: HashMap<MeasureNameId, JsonV1Measure>,
}

/// A measure's metrics. Every name is equal on the wire.
pub type JsonV1Measure = NamedMap;

/// The BMF v1 leaf of the `json` adapter tree.
///
/// All or nothing: every benchmark in the payload must map to an array, so a
/// payload that mixes v0 objects and v1 arrays fails here and at `json_v0`, and
/// therefore at the `json` node.
pub struct AdapterJsonV1;

impl Adaptable for AdapterJsonV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let results: JsonV1Results = serde_json::from_str(input).ok()?;
        Some(from_wire(results))
    }
}

fn from_wire(results: JsonV1Results) -> AdapterResults {
    let mut dropped_names = 0;
    let mut results_map = ResultsMap::with_capacity(results.len());
    for (benchmark, entries) in results {
        let mut benchmark_entries = BenchmarkEntries::new();
        for JsonV1Entry {
            parameters,
            measures,
        } in entries
        {
            // Two entries that canonicalize to the same parameter set are one
            // variant, so their metrics merge rather than fork a series. A name
            // that genuinely repeats takes the later entry, which is deterministic
            // because entries are an ordered array in wire order. Nothing is
            // dropped, so nothing is counted: a harness that emits one entry per
            // statistic is a plausible shape, not an error.
            let metrics: &mut AdapterMetrics = benchmark_entries.entry(parameters).or_default();
            for (measure, named) in measures {
                metrics
                    .inner
                    .entry(measure)
                    .or_default()
                    .inner
                    .extend(named);
            }
        }
        // The cap applies to the merged variant, so a name is counted once.
        for metrics in benchmark_entries.values_mut() {
            for metric in metrics.inner.values_mut() {
                dropped_names += metric.truncate();
            }
        }
        results_map.insert(benchmark, benchmark_entries);
    }

    AdapterResults {
        inner: results_map,
        version: BmfVersion::V1,
        dropped_names,
    }
}

#[cfg(test)]
pub(crate) mod test_json_v1 {
    use bencher_json::{BenchmarkNameId, MAX_PARAMETER_KEYS, MetricName, ParameterSet};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterJsonV1;
    use crate::{
        Adaptable as _, Settings,
        adapters::{
            json::v0::AdapterJsonV0,
            test_util::{convert_file_path, opt_convert_file_path},
        },
        results::{
            adapter_metrics::{AdapterMetrics, MAX_METRIC_NAMES},
            adapter_results::{AdapterResults, BmfVersion},
        },
    };

    fn convert_json_v1(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{suffix}.json");
        convert_file_path::<AdapterJsonV1>(&file_path)
    }

    /// A payload a test spells out inline, because the merge it pins is a
    /// property of the payload rather than of any tool's output.
    fn parse_json_v1(input: &str) -> AdapterResults {
        AdapterJsonV1::parse(input, Settings::default())
            .unwrap_or_else(|| panic!("Failed to parse {input}"))
    }

    /// The metrics of one benchmark's variant.
    pub fn variant<'r>(
        results: &'r AdapterResults,
        benchmark: &str,
        parameters: &str,
    ) -> &'r AdapterMetrics {
        let benchmark = benchmark
            .parse::<BenchmarkNameId>()
            .expect("Failed to parse benchmark name");
        let parameters = parameters
            .parse::<ParameterSet>()
            .expect("Failed to parse parameters");
        results
            .inner
            .get(&benchmark)
            .expect("Missing benchmark")
            .get(&parameters)
            .expect("Missing parameter set")
    }

    /// One metric of one measure.
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

        let metrics = variant(
            results,
            "tests::benchmark_a",
            r#"{"size_mb": 16, "op": "read"}"#,
        );
        assert_eq!(named(metrics, "latency", "value"), Some(42.0.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(97.0.into()));
        assert_eq!(named(metrics, "latency", "p50"), Some(40.5.into()));

        let metrics = variant(
            results,
            "tests::benchmark_a",
            r#"{"size_mb": 32, "op": "read"}"#,
        );
        assert_eq!(named(metrics, "latency", "value"), Some(55.1.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(120.3.into()));
        assert_eq!(named(metrics, "latency", "p50"), Some(51.9.into()));
    }

    /// An entry without `parameters` resolves to the empty parameter set,
    /// which is exactly what an explicit `{}` resolves to: one variant, not two.
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

        // An absent `parameters` and an explicit `{}` merge into one variant.
        let merged = "tests::merged".parse::<BenchmarkNameId>().unwrap();
        let entries = &results.inner[&merged];
        assert_eq!(entries.len(), 1);
        let metrics = &entries[&ParameterSet::default()];
        assert_eq!(named(metrics, "latency", "value"), Some(1.0.into()));
        assert_eq!(named(metrics, "throughput", "value"), Some(2.0.into()));
    }

    /// Metrics are scalars and every name is equal:
    /// a measure may carry only `p99` and never mention `value`.
    #[test]
    fn adapter_json_v1_metrics() {
        let results = convert_json_v1("v1_named");
        validate_adapter_json_v1_named(&results);
    }

    pub fn validate_adapter_json_v1_named(results: &AdapterResults) {
        let metrics = variant(results, "tests::percentiles", "{}");
        assert_eq!(names(metrics, "latency"), vec!["p99".parse().unwrap()]);
        assert_eq!(named(metrics, "latency", "p99"), Some(97.0.into()));
        assert_eq!(named(metrics, "latency", "value"), None);
    }

    /// Two entries that resolve to the same variant union their metrics.
    /// Nothing is dropped, so nothing is counted, and a name that genuinely repeats
    /// takes the later entry's value, deterministically, because entries are an
    /// ordered array in wire order.
    #[test]
    fn adapter_json_v1_duplicate_variants_union_names() {
        let input = r#"{"bench": [
            {"measures": {"latency": {"p50": 1.0, "value": 3.0}}},
            {"measures": {"latency": {"p99": 2.0, "value": 4.0}}}
        ]}"#;
        let results = AdapterJsonV1::parse(input, Settings::default())
            .expect("Failed to parse the duplicate variants");

        let metrics = variant(&results, "bench", "{}");
        assert_eq!(
            names(metrics, "latency"),
            vec![
                "p50".parse::<MetricName>().unwrap(),
                "p99".parse().unwrap(),
                "value".parse().unwrap(),
            ],
            "both entries' names survive"
        );
        assert_eq!(named(metrics, "latency", "p50"), Some(1.0.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(2.0.into()));
        assert_eq!(
            named(metrics, "latency", "value"),
            Some(4.0.into()),
            "a repeated name takes the later entry"
        );
        assert_eq!(results.dropped_names, 0, "nothing was dropped");
    }

    /// The union is per measure: a measure only one entry mentions is untouched.
    #[test]
    fn adapter_json_v1_duplicate_variants_union_measures() {
        let input = r#"{"bench": [
            {"measures": {"latency": {"value": 1.0}}},
            {"measures": {"throughput": {"value": 2.0}}}
        ]}"#;
        let results = AdapterJsonV1::parse(input, Settings::default())
            .expect("Failed to parse the duplicate variants");

        let metrics = variant(&results, "bench", "{}");
        assert_eq!(named(metrics, "latency", "value"), Some(1.0.into()));
        assert_eq!(named(metrics, "throughput", "value"), Some(2.0.into()));
    }

    /// Two entries whose parameters differ only in key order or number spelling
    /// are the same variant, and the measures of all three union by name.
    #[test]
    fn adapter_json_v1_canonical_parameters() {
        let results = convert_json_v1("v1_canonical");
        validate_adapter_json_v1_canonical(&results);
    }

    pub fn validate_adapter_json_v1_canonical(results: &AdapterResults) {
        let benchmark = "tests::canonical".parse::<BenchmarkNameId>().unwrap();
        let entries = &results.inner[&benchmark];
        assert_eq!(entries.len(), 1);

        let metrics = variant(results, "tests::canonical", r#"{"op":"read","size_mb":16}"#);
        // Every entry's names survive and the last entry wins the one they share.
        assert_eq!(
            names(metrics, "latency"),
            vec![
                "p50".parse::<MetricName>().unwrap(),
                "p90".parse().unwrap(),
                "p99".parse().unwrap(),
                "value".parse().unwrap(),
            ]
        );
        assert_eq!(named(metrics, "latency", "p50"), Some(10.0.into()));
        assert_eq!(named(metrics, "latency", "p90"), Some(20.0.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(30.0.into()));
        assert_eq!(named(metrics, "latency", "value"), Some(3.0.into()));
        assert_eq!(named(metrics, "throughput", "value"), Some(100.0.into()));
        assert_eq!(results.dropped_names, 0);
    }

    /// Two entries of one canonical parameter set union the names of the
    /// measure they share, rather than the later entry replacing the earlier.
    #[test]
    fn adapter_json_v1_duplicate_entries_union_names() {
        let results = parse_json_v1(
            r#"{"tests::disjoint": [
                {"measures": {"latency": {"value": 1}}},
                {"measures": {"latency": {"p99": 2}}}
            ]}"#,
        );

        let metrics = variant(&results, "tests::disjoint", "{}");
        assert_eq!(named(metrics, "latency", "value"), Some(1.0.into()));
        assert_eq!(named(metrics, "latency", "p99"), Some(2.0.into()));
        assert_eq!(results.dropped_names, 0);
    }

    /// A name written by more than one entry takes the last entry's value,
    /// exactly JSON object key semantics, and its neighbors are untouched.
    #[test]
    fn adapter_json_v1_duplicate_entries_later_name_wins() {
        let results = parse_json_v1(
            r#"{"tests::overlap": [
                {"measures": {"latency": {"value": 1, "p99": 2}}},
                {"measures": {"latency": {"p99": 3}}}
            ]}"#,
        );

        let metrics = variant(&results, "tests::overlap", "{}");
        assert_eq!(named(metrics, "latency", "p99"), Some(3.0.into()));
        assert_eq!(named(metrics, "latency", "value"), Some(1.0.into()));
        assert_eq!(results.dropped_names, 0);
    }

    /// The cap applies to the union rather than to either entry alone,
    /// keeping the conventional trio and the rest lexicographically.
    #[test]
    fn adapter_json_v1_cap_applies_to_the_union() {
        let results = parse_json_v1(
            r#"{"tests::capped": [
                {"measures": {"latency": {"a1": 1, "a2": 2, "a3": 3, "a4": 4, "a5": 5}}},
                {"measures": {"latency": {"a6": 6, "a7": 7, "value": 8, "lower_value": 9, "upper_value": 10}}}
            ]}"#,
        );

        let metrics = variant(&results, "tests::capped", "{}");
        assert_eq!(
            names(metrics, "latency"),
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

    /// A name written by two entries is one name, never a drop: nine metrics
    /// across two entries are eight names and nothing is counted.
    #[test]
    fn adapter_json_v1_overwrite_is_not_a_drop() {
        let results = parse_json_v1(
            r#"{"tests::overwritten": [
                {"measures": {"latency": {"a1": 1, "a2": 2, "a3": 3, "a4": 4, "a5": 5, "value": 8}}},
                {"measures": {"latency": {"value": 9, "lower_value": 10, "upper_value": 11}}}
            ]}"#,
        );

        let metrics = variant(&results, "tests::overwritten", "{}");
        assert_eq!(names(metrics, "latency").len(), MAX_METRIC_NAMES);
        assert_eq!(named(metrics, "latency", "a1"), Some(1.0.into()));
        assert_eq!(named(metrics, "latency", "value"), Some(9.0.into()));
        assert_eq!(results.dropped_names, 0);
    }

    /// The cap keeps the three conventional names regardless of where they sort,
    /// keeps the rest lexicographically, and reports what it dropped.
    #[test]
    fn adapter_json_v1_metric_cap() {
        let results = convert_json_v1("v1_cap");
        validate_adapter_json_v1_cap(&results);
    }

    pub fn validate_adapter_json_v1_cap(results: &AdapterResults) {
        let metrics = variant(results, "tests::capped", "{}");
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
        let metrics = variant(&permuted, "tests::capped", "{}");
        assert_eq!(
            names(metrics, "latency"),
            names(variant(&expected, "tests::capped", "{}"), "latency")
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

    /// A benchmark that reports zero entries is a well formed v1 benchmark that
    /// measured nothing. It is accepted, it is not an error, and it yields no
    /// variant and therefore no metric.
    #[test]
    fn adapter_json_v1_empty_entries() {
        let results = parse_json_v1(r#"{"tests::none": []}"#);

        let benchmark = "tests::none".parse::<BenchmarkNameId>().unwrap();
        assert!(results.inner[&benchmark].is_empty());
        assert_eq!(results.dropped_names, 0);
    }

    /// An entry that names no measure is a variant that measured nothing. It is
    /// accepted, it is not an error, and it yields no metric. This is exactly the
    /// v0 shape `{"bench": {}}`, which has always parsed to one variant on the
    /// empty parameter set with no measures under it.
    #[test]
    fn adapter_json_v1_empty_measures() {
        let results = parse_json_v1(r#"{"tests::none": [{"measures": {}}]}"#);

        let metrics = variant(&results, "tests::none", "{}");
        assert!(metrics.inner.is_empty());
        assert_eq!(results.dropped_names, 0);

        // The v0 analog, byte for byte the same variant.
        let v0 = AdapterJsonV0::parse(r#"{"tests::none": {}}"#, Settings::default())
            .expect("Failed to parse the v0 payload");
        assert_eq!(results.inner, v0.inner);
    }

    /// An entry that names a parameter set but no measure still resolves that
    /// parameter set: the variant exists, it just measured nothing.
    #[test]
    fn adapter_json_v1_empty_measures_keeps_the_parameter_set() {
        let results =
            parse_json_v1(r#"{"tests::none": [{"parameters": {"size_mb": 16}, "measures": {}}]}"#);

        let metrics = variant(&results, "tests::none", r#"{"size_mb":16}"#);
        assert!(metrics.inner.is_empty());
        assert_eq!(results.dropped_names, 0);
    }

    /// A parameter set is bounded: at most `MAX_PARAMETER_KEYS` keys, and a key
    /// or a string value that is non-empty, trimmed, and within `MAX_LEN` bytes.
    ///
    /// A payload that breaks a bound is not a v1 payload. The whole report fails
    /// to parse, so the run is rejected rather than quietly losing the variant
    /// that carried the offending set.
    #[test]
    fn adapter_json_v1_rejects_out_of_bounds_parameters() {
        fn entry(parameters: &str) -> String {
            format!(
                r#"{{"bench": [{{"parameters": {parameters}, "measures": {{"latency": {{"value": 1}}}}}}]}}"#
            )
        }
        fn keys(count: usize) -> String {
            format!(
                "{{{}}}",
                (0..count)
                    .map(|index| format!(r#""k{index}": {index}"#))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }

        let at_max_len = "0".repeat(64);
        let over_max_len = "0".repeat(65);

        for parameters in [
            keys(MAX_PARAMETER_KEYS + 1),
            format!(r#"{{"{over_max_len}": 1}}"#),
            format!(r#"{{"a": "{over_max_len}"}}"#),
            r#"{"": 1}"#.to_owned(),
            r#"{" a": 1}"#.to_owned(),
            r#"{"a": " b"}"#.to_owned(),
            r#"{"a": ""}"#.to_owned(),
        ] {
            assert!(
                AdapterJsonV1::parse(&entry(&parameters), Settings::default()).is_none(),
                "expected {parameters} to be rejected"
            );
        }

        // Every bound is inclusive, so a set sitting on each limit still parses.
        for parameters in [
            keys(MAX_PARAMETER_KEYS),
            format!(r#"{{"{at_max_len}": 1}}"#),
            format!(r#"{{"a": "{at_max_len}"}}"#),
        ] {
            assert!(
                AdapterJsonV1::parse(&entry(&parameters), Settings::default()).is_some(),
                "expected {parameters} to be accepted"
            );
        }
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
