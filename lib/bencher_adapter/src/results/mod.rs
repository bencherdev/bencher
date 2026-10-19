use std::collections::BTreeSet;

use bencher_json::{BmfVersion, MeasureNameId, project::report::Adapter};

use crate::{
    Adaptable as _, AdapterError, Settings,
    adapters::json::{v0::AdapterJsonV0, v1::AdapterJsonV1},
};

pub mod adapter_metrics;
pub mod adapter_results;
pub mod foldable;
pub mod results_reducer;

use adapter_results::{AdapterResults, Skipped};
use foldable::FoldableResultsArray;

#[derive(Debug, Clone)]
pub struct AdapterResultsArray {
    pub inner: ResultsArray,
}

pub type ResultsArray = Vec<AdapterResults>;

impl AdapterResultsArray {
    pub fn new(
        results_array: &[&str],
        adapter: Adapter,
        settings: Settings,
    ) -> Result<Self, AdapterError> {
        let mut parsed_results_array = Vec::new();
        for &results in results_array {
            let parsed_results = adapter
                .convert(results, settings)
                .ok_or_else(|| Self::refusal(results, adapter, settings.bmf_version))?;
            // The declared version is a contract over the whole payload.
            if parsed_results.version != settings.bmf_version {
                return Err(AdapterError::BmfVersion {
                    declared: settings.bmf_version,
                    parsed: parsed_results.version,
                });
            }
            parsed_results_array.push(parsed_results);
        }
        Ok(Self::from_iterations(parsed_results_array))
    }

    /// The error for a payload the adapter did not claim, where under `json` or
    /// `magic` the other version's leaf is probed only to name the version and
    /// nothing ingests through it.
    fn refusal(results: &str, adapter: Adapter, declared: BmfVersion) -> AdapterError {
        let probe = if !matches!(adapter, Adapter::Json | Adapter::Magic) {
            None
        } else if declared == BmfVersion::V0 {
            AdapterJsonV1::parse(results, Settings::default())
        } else {
            AdapterJsonV0::parse(results, Settings::default())
        };
        match probe {
            Some(other) => AdapterError::BmfVersion {
                declared,
                parsed: other.version,
            },
            None => AdapterError::Convert(results.to_owned()),
        }
    }

    /// The report's results, one per iteration, each cut to the per measure cap.
    ///
    /// Every leaf's results pass through here, so no leaf can miss the cap.
    pub fn from_iterations(mut inner: ResultsArray) -> Self {
        for results in &mut inner {
            results.truncate();
        }
        Self { inner }
    }

    /// The leaf adapter the first iteration resolved to, which the report records.
    pub fn resolved_adapter(&self) -> Option<Adapter> {
        self.inner.first()?.resolved_adapter
    }

    /// How many iterations resolved to a leaf adapter other than the first one's.
    pub fn mixed_adapters(&self) -> usize {
        let resolved_adapter = self.resolved_adapter();
        self.inner
            .iter()
            .filter(|results| results.resolved_adapter != resolved_adapter)
            .count()
    }

    /// Whether any iteration holds a metric.
    pub fn has_metrics(&self) -> bool {
        self.inner.iter().any(AdapterResults::has_metrics)
    }

    /// Every measure any iteration holds, once each.
    pub fn measures(&self) -> BTreeSet<&MeasureNameId> {
        self.inner
            .iter()
            .flat_map(AdapterResults::measures)
            .collect()
    }

    /// Keep only the measures `keep` holds to, and drop each variant and benchmark that held
    /// measures and holds none once they are gone.
    pub fn retain_measures<F>(&mut self, keep: F)
    where
        F: Fn(&MeasureNameId) -> bool,
    {
        for results in &mut self.inner {
            results.retain_measures(&keep);
        }
    }

    pub fn skipped(&self) -> Skipped {
        self.inner
            .iter()
            .fold(Skipped::default(), |sum, results| sum + results.skipped)
    }

    /// Every result as a foldable BMF v0 payload, or the array back untouched if
    /// any member is BMF v1.
    ///
    /// Fold is not supported for BMF v1, so a caller handed its array back
    /// ingests unfolded, one iteration per payload.
    pub fn foldable(self) -> Result<FoldableResultsArray, Self> {
        if !self.inner.iter().all(AdapterResults::is_foldable) {
            return Err(self);
        }

        Ok(FoldableResultsArray {
            inner: self
                .inner
                .into_iter()
                .map(AdapterResults::into_foldable)
                .collect(),
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum OrdKind {
    Min,
    Max,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CombinedKind {
    Ord(OrdKind),
    Add,
}

#[cfg(test)]
mod test_results_array {
    use std::collections::BTreeMap;

    use bencher_json::{
        BenchmarkNameId, BmfVersion, MeasureNameId, MetricName, ParameterSet,
        project::report::Adapter,
    };

    use super::{
        AdapterResultsArray,
        adapter_metrics::{AdapterMetric, AdapterMetrics, MetricsMap, NamedMap},
        adapter_results::ResultsMap,
    };
    use crate::{
        AdapterError, AdapterResults, Settings,
        adapters::json::v1::test_json_v1::{named, names, variant},
    };

    const V0: &str = r#"{"bench": {"latency": {"value": 10.0}}}"#;
    const V1: &str = r#"{"bench": [{"measures": {"latency": {"value": 10.0}}}]}"#;

    fn new_at(results: &str, adapter: Adapter, bmf_version: BmfVersion) -> AdapterError {
        AdapterResultsArray::new(&[results], adapter, Settings::new(None, bmf_version))
            .expect_err("expected the results array to refuse the payload")
    }

    fn assert_contract(error: &AdapterError, declared: BmfVersion, parsed: BmfVersion) {
        match error {
            AdapterError::BmfVersion {
                declared: error_declared,
                parsed: error_parsed,
            } => {
                assert_eq!(*error_declared, declared);
                assert_eq!(*error_parsed, parsed);
            },
            AdapterError::Valid(_) | AdapterError::BenchmarkUnits(_) | AdapterError::Convert(_) => {
                panic!("expected the BMF version error: {error}")
            },
        }
    }

    /// The `json` node parses no leaf but the declared one, and the refusal still
    /// names the version the payload is written in.
    #[test]
    fn results_array_json_refuses_the_other_version_by_name() {
        for adapter in [Adapter::Json, Adapter::Magic] {
            let error = new_at(V0, adapter, BmfVersion::V1);
            assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
            let error = new_at(V1, adapter, BmfVersion::V0);
            assert_contract(&error, BmfVersion::V0, BmfVersion::V1);
        }
    }

    /// A payload no leaf claims fails to convert at either version.
    #[test]
    fn results_array_json_unclaimed_payload_fails_to_convert() {
        let results = std::fs::read_to_string("./tool_output/json/report_mixed_versions.json")
            .expect("Failed to read test file");
        for adapter in [Adapter::Json, Adapter::Magic] {
            for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
                let error = new_at(&results, adapter, bmf_version);
                assert!(
                    matches!(error, AdapterError::Convert(_)),
                    "expected the convert error at version {bmf_version}: {error}"
                );
            }
        }
    }

    /// The `json_v1` leaf parses a v1 payload whatever was declared, so declaring
    /// version 0 breaks the contract.
    #[test]
    fn results_array_json_v1_refuses_version_0() {
        let error = new_at(V1, Adapter::JsonV1, BmfVersion::V0);
        assert_contract(&error, BmfVersion::V0, BmfVersion::V1);
    }

    /// The `json_v0` leaf parses a v0 payload whatever was declared, so declaring
    /// version 1 breaks the contract.
    #[test]
    fn results_array_json_v0_refuses_version_1() {
        let error = new_at(V0, Adapter::JsonV0, BmfVersion::V1);
        assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
    }

    /// A harness adapter the version 1 tree does not hold yet parses as version 0,
    /// so its output at version 1 breaks the contract.
    #[test]
    fn results_array_non_bmf_adapter_refuses_version_1() {
        let results = std::fs::read_to_string("./tool_output/go/bench/five.txt")
            .expect("Failed to read test file");
        let error = new_at(&results, Adapter::GoBench, BmfVersion::V1);
        assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
        AdapterResultsArray::new(
            &[&results],
            Adapter::GoBench,
            Settings::new(None, BmfVersion::V0),
        )
        .expect("a non-BMF adapter parses at version 0");
    }

    /// A harness payload declared at version 1 is refused by the contract
    /// through `magic` too, since the version 1 tree falls back to the version 0
    /// tree for every harness it does not hold yet.
    #[test]
    fn results_array_magic_refuses_a_harness_payload_at_version_1() {
        let results = std::fs::read_to_string("./tool_output/rust/bench/many.txt")
            .expect("Failed to read test file");
        let error = new_at(&results, Adapter::Magic, BmfVersion::V1);
        assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
    }

    /// The report records the first iteration's leaf, and counts every later
    /// iteration that resolved to another.
    #[test]
    fn results_array_resolves_to_the_first_leaf() {
        let array = resolved([
            Some(Adapter::JsonV1),
            Some(Adapter::RustCriterion),
            Some(Adapter::JsonV1),
            Some(Adapter::RustBench),
        ]);
        assert_eq!(array.resolved_adapter(), Some(Adapter::JsonV1));
        assert_eq!(array.mixed_adapters(), 2);

        let array = resolved([Some(Adapter::JsonV1); 3]);
        assert_eq!(array.resolved_adapter(), Some(Adapter::JsonV1));
        assert_eq!(array.mixed_adapters(), 0);

        let array = resolved([None; 2]);
        assert_eq!(array.resolved_adapter(), None);
        assert_eq!(array.mixed_adapters(), 0);

        let array = resolved([]);
        assert_eq!(array.resolved_adapter(), None);
        assert_eq!(array.mixed_adapters(), 0);
    }

    /// Results hold a metric only when some measure names one: a benchmark, a
    /// variant, or a measure with nothing under it is still empty.
    #[test]
    fn results_array_has_metrics_only_with_a_metric() {
        for (results, bmf_version, has_metrics) in [
            ("{}", BmfVersion::V0, false),
            ("{}", BmfVersion::V1, false),
            (r#"{"bench": {}}"#, BmfVersion::V0, false),
            (r#"{"bench": []}"#, BmfVersion::V1, false),
            (r#"{"bench": [{"measures": {}}]}"#, BmfVersion::V1, false),
            (
                r#"{"bench": [{"measures": {"latency": {}}}]}"#,
                BmfVersion::V1,
                false,
            ),
            (V0, BmfVersion::V0, true),
            (V1, BmfVersion::V1, true),
        ] {
            let array = AdapterResultsArray::new(
                &[results, "{}"],
                Adapter::Json,
                Settings::new(None, bmf_version),
            )
            .unwrap_or_else(|e| panic!("expected {results} to parse: {e}"));
            assert_eq!(array.has_metrics(), has_metrics, "{results}");
        }
    }

    /// A drop keeps the listed measures, drops each variant and benchmark it empties, and
    /// leaves a variant or benchmark that held no measure before it as it was.
    ///
    /// Kills: a retain that keeps an unlisted measure, leaves an emptied variant or benchmark
    /// behind to write its row, or drops a variant or benchmark that was already empty.
    #[test]
    fn results_array_retains_the_kept_measures() {
        let v0 = r#"{
            "kept": {"latency": {"value": 1.0}, "throughput": {"value": 2.0}},
            "dropped": {"throughput": {"value": 3.0}},
            "empty": {}
        }"#;
        let v1 = r#"{
            "kept": [
                {"measures": {"latency": {"value": 1.0}, "throughput": {"value": 2.0}}},
                {"parameters": {"size": 1}, "measures": {"throughput": {"value": 3.0}}},
                {"parameters": {"size": 2}, "measures": {}}
            ],
            "dropped": [{"measures": {"throughput": {"value": 4.0}}}],
            "empty": []
        }"#;
        let empty_variant = |parameters: &str| {
            (
                serde_json::from_str::<ParameterSet>(parameters)
                    .unwrap()
                    .canonical(),
                Vec::new(),
            )
        };
        let latency = "latency".parse::<MeasureNameId>().unwrap();
        for (results, bmf_version, extra) in [
            (v0, BmfVersion::V0, None),
            (v1, BmfVersion::V1, Some(empty_variant(r#"{"size": 2}"#))),
        ] {
            let mut array = AdapterResultsArray::new(
                &[results],
                Adapter::Json,
                Settings::new(None, bmf_version),
            )
            .unwrap_or_else(|e| panic!("expected {results} to parse: {e}"));
            let before = shape(&array);
            array.retain_measures(|measure| *measure == latency);

            let kept = std::iter::once(("{}".to_owned(), vec!["latency".to_owned()]))
                .chain(extra)
                .collect();
            let expected = [
                ("empty".to_owned(), before["empty"].clone()),
                ("kept".to_owned(), kept),
            ]
            .into_iter()
            .collect::<Shape>();
            assert_eq!(shape(&array), expected, "{bmf_version}");
        }
    }

    /// Each benchmark's variants, by canonical parameters, with their measures.
    type Shape = BTreeMap<String, BTreeMap<String, Vec<String>>>;

    fn shape(array: &AdapterResultsArray) -> Shape {
        let [results] = array.inner.as_slice() else {
            panic!("expected one iteration");
        };
        results
            .inner
            .iter()
            .map(|(benchmark, entries)| {
                let variants = entries
                    .iter()
                    .map(|(parameters, metrics)| {
                        let mut measures = metrics
                            .inner
                            .keys()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>();
                        measures.sort();
                        (parameters.canonical(), measures)
                    })
                    .collect();
                (benchmark.to_string(), variants)
            })
            .collect()
    }

    fn resolved<const N: usize>(adapters: [Option<Adapter>; N]) -> AdapterResultsArray {
        let inner = adapters
            .into_iter()
            .map(|resolved_adapter| AdapterResults {
                resolved_adapter,
                ..AdapterResults::default()
            })
            .collect::<Vec<_>>();
        AdapterResultsArray::from_iterations(inner)
    }

    /// The contract error names both versions, since that is what a client reads.
    #[test]
    fn results_array_contract_error_names_both_versions() {
        let error = new_at(V1, Adapter::JsonV1, BmfVersion::V0);
        assert_eq!(
            error.to_string(),
            "Results parsed as BMF version 1 but the payload declared version 0"
        );
    }

    /// The cap cuts results no leaf built, and counts the cut on their own iteration.
    #[test]
    fn results_array_cuts_results_built_directly() {
        let metric = (1..=14u8)
            .map(|number| format!("a{number:02}"))
            .chain(["value", "lower_value", "upper_value"].map(str::to_owned))
            .zip(1u8..)
            .map(|(name, number)| (name.parse().unwrap(), f64::from(number).into()))
            .collect::<NamedMap>();
        let metrics = AdapterMetrics::from(
            std::iter::once((
                "latency".parse::<MeasureNameId>().unwrap(),
                AdapterMetric::from(metric),
            ))
            .collect::<MetricsMap>(),
        );
        let results = AdapterResults::from(
            std::iter::once((
                "tests::capped".parse::<BenchmarkNameId>().unwrap(),
                std::iter::once((ParameterSet::default(), metrics)).collect(),
            ))
            .collect::<ResultsMap>(),
        );

        let array = AdapterResultsArray::from_iterations(vec![AdapterResults::default(), results]);
        let [empty, capped] = array.inner.as_slice() else {
            panic!("expected two iterations");
        };
        let metrics = variant(capped, "tests::capped", "{}");
        assert_eq!(names(metrics, "latency"), capped_names());
        assert_eq!(capped.skipped.metrics, 1);
        assert_eq!(empty.skipped.metrics, 0);
    }

    /// The cap keeps the three conventional names regardless of where they sort,
    /// keeps the rest lexicographically up to sixteen, and counts what it dropped,
    /// whichever adapter reached the leaf.
    #[test]
    fn results_array_metric_cap() {
        let results = fixture("v1_cap");
        for adapter in [Adapter::JsonV1, Adapter::Json, Adapter::Magic] {
            let capped = only(&v1_array(&results, adapter));
            let metrics = variant(&capped, "tests::capped", "{}");
            assert_eq!(names(metrics, "latency"), capped_names(), "{adapter}");
            assert_eq!(capped.skipped.metrics, 1, "{adapter}");
        }
    }

    /// Sixteen names, the most a measure keeps, all kept and none counted.
    #[test]
    fn results_array_cap_keeps_sixteen() {
        let results = format!(
            r#"{{"tests::full": [{{"measures": {{"latency": {}}}}}]}}"#,
            numbered(1..=13, &["value", "lower_value", "upper_value"]),
        );
        let full = only(&v1_array(&results, Adapter::JsonV1));

        let metrics = variant(&full, "tests::full", "{}");
        assert_eq!(names(metrics, "latency"), capped_names());
        assert_eq!(full.skipped.metrics, 0);
    }

    /// The cap cuts every measure of every variant of every benchmark, and counts every cut.
    #[test]
    fn results_array_cap_cuts_every_measure() {
        let over = serde_json::from_str::<serde_json::Value>(&numbered(
            1..=14,
            &["value", "lower_value", "upper_value"],
        ))
        .unwrap();
        let entry = |size: u8| {
            serde_json::json!({
                "parameters": { "size": size },
                "measures": { "latency": over, "throughput": over },
            })
        };
        let results = serde_json::json!({
            "tests::first": [entry(1), entry(2)],
            "tests::second": [entry(1), entry(2)],
        })
        .to_string();
        let capped = only(&v1_array(&results, Adapter::JsonV1));

        for benchmark in ["tests::first", "tests::second"] {
            for parameters in [r#"{"size": 1}"#, r#"{"size": 2}"#] {
                let metrics = variant(&capped, benchmark, parameters);
                for measure in ["latency", "throughput"] {
                    assert_eq!(
                        names(metrics, measure),
                        capped_names(),
                        "{benchmark} {parameters} {measure}"
                    );
                }
            }
        }
        assert_eq!(capped.skipped.metrics, 8);
    }

    /// A name the leaf skips and a name the cap cuts on one measure both count.
    #[test]
    fn results_array_counts_skips_and_cuts_together() {
        let mut latency = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
            &numbered(1..=14, &["value", "lower_value", "upper_value"]),
        )
        .unwrap();
        latency.insert(
            "n".repeat(MetricName::MAX_LEN + 1),
            serde_json::Value::from(0),
        );
        let results = serde_json::json!({
            "tests::capped": [{ "measures": { "latency": latency } }],
        })
        .to_string();
        let capped = only(&v1_array(&results, Adapter::JsonV1));

        let metrics = variant(&capped, "tests::capped", "{}");
        assert_eq!(names(metrics, "latency"), capped_names());
        assert_eq!(capped.skipped.metrics, 2);
    }

    /// The cap applies to the merged variant rather than to either entry alone,
    /// keeping the conventional trio and the rest lexicographically.
    #[test]
    fn results_array_cap_applies_to_the_merged_variant() {
        let results = format!(
            r#"{{"tests::capped": [
                {{"measures": {{"latency": {}}}}},
                {{"measures": {{"latency": {}}}}}
            ]}}"#,
            numbered(1..=10, &[]),
            numbered(11..=14, &["value", "lower_value", "upper_value"]),
        );
        let capped = only(&v1_array(&results, Adapter::JsonV1));

        let metrics = variant(&capped, "tests::capped", "{}");
        assert_eq!(names(metrics, "latency"), capped_names());
        assert_eq!(capped.skipped.metrics, 1);
    }

    /// A name written by two entries is one name, never a drop: seventeen metrics
    /// across two entries are sixteen names and nothing is counted.
    #[test]
    fn results_array_overwrite_is_not_a_drop() {
        let results = format!(
            r#"{{"tests::overwritten": [
                {{"measures": {{"latency": {}}}}},
                {{"measures": {{"latency": {{"value": 99, "lower_value": 98, "upper_value": 97}}}}}}
            ]}}"#,
            numbered(1..=13, &["value"]),
        );
        let overwritten = only(&v1_array(&results, Adapter::JsonV1));

        let metrics = variant(&overwritten, "tests::overwritten", "{}");
        assert_eq!(names(metrics, "latency").len(), 16);
        assert_eq!(named(metrics, "latency", "a01"), Some(1.0.into()));
        assert_eq!(named(metrics, "latency", "value"), Some(99.0.into()));
        assert_eq!(overwritten.skipped.metrics, 0);
    }

    /// The survivor set is a property of the payload, not of hash iteration order.
    #[test]
    fn results_array_cap_is_deterministic() {
        let results = fixture("v1_cap");
        let expected = only(&v1_array(&results, Adapter::JsonV1));
        for _ in 0..64u32 {
            assert_eq!(only(&v1_array(&results, Adapter::JsonV1)), expected);
        }

        // The same names in a different order on the wire keep the same survivors.
        let permuted = only(&v1_array(&fixture("v1_cap_permuted"), Adapter::JsonV1));
        let metrics = variant(&permuted, "tests::capped", "{}");
        assert_eq!(
            names(metrics, "latency"),
            names(variant(&expected, "tests::capped", "{}"), "latency")
        );
        assert_eq!(permuted.skipped.metrics, 1);
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/json/report_{suffix}.json"))
            .expect("Failed to read test file")
    }

    fn v1_array(results: &str, adapter: Adapter) -> AdapterResultsArray {
        AdapterResultsArray::new(&[results], adapter, Settings::new(None, BmfVersion::V1))
            .unwrap_or_else(|e| panic!("expected {adapter} to parse the payload: {e}"))
    }

    fn only(array: &AdapterResultsArray) -> AdapterResults {
        let [results] = array.inner.as_slice() else {
            panic!("expected one iteration");
        };
        results.clone()
    }

    /// A measure's metrics: `a01` up to the last of `numbers`, then each of `conventional`.
    fn numbered(numbers: std::ops::RangeInclusive<u8>, conventional: &[&str]) -> String {
        let metrics = numbers
            .map(|number| (format!("a{number:02}"), serde_json::json!(number)))
            .chain(
                conventional
                    .iter()
                    .zip(15u8..)
                    .map(|(name, number)| ((*name).to_owned(), serde_json::json!(number))),
            )
            .collect::<serde_json::Map<_, _>>();
        serde_json::Value::Object(metrics).to_string()
    }

    /// The sixteen names the cap keeps out of `a01` to `a14` and the conventional trio:
    /// the trio, and the rest lexicographically, so `a14` is the one cut.
    fn capped_names() -> Vec<MetricName> {
        (1..=13u8)
            .map(|number| format!("a{number:02}"))
            .chain(["lower_value", "upper_value", "value"].map(str::to_owned))
            .map(|name| name.parse().unwrap())
            .collect()
    }
}
