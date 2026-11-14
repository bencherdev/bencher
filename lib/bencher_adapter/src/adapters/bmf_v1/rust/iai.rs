use bencher_json::{ParameterSet, project::report::Adapter};

use crate::{
    Adaptable, Settings,
    adapters::{bmf_v1::leaf::LeafResults, rust::iai::parse_iai},
    results::adapter_results::AdapterResults,
};

/// iai at version 1: each of its five counters is an exact count, so `value`.
pub struct AdapterRustIaiV1;

impl Adaptable for AdapterRustIaiV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for (benchmark_name, measures) in parse_iai(input) {
            let mut variant = results.variant(benchmark_name, ParameterSet::default());
            for measure in measures {
                let (measure, metric) = measure.into_measure();
                variant.insert(measure, "value", metric.value.into_inner());
            }
        }
        results.finish(Adapter::RustIai)
    }
}

#[cfg(test)]
mod test_rust_iai_v1 {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            iai::{EstimatedCycles, Instructions, L1Accesses, L2Accesses, RamAccesses},
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{assert_ignores_average, assert_variant, convert_v1};

    fn counters(values: [f64; 5]) -> Vec<(&'static str, Vec<(&'static str, f64)>)> {
        [
            Instructions::SLUG_STR,
            L1Accesses::SLUG_STR,
            L2Accesses::SLUG_STR,
            RamAccesses::SLUG_STR,
            EstimatedCycles::SLUG_STR,
        ]
        .into_iter()
        .zip(values)
        .map(|(measure, value)| (measure, vec![("value", value)]))
        .collect()
    }

    /// Kills a leaf that drops a counter, promotes another name, or records no leaf.
    #[test]
    fn adapter_rust_iai_v1_cache_sim() {
        let results = convert_v1(Adapter::RustIai, "rust/iai/cache_sim.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustIai));
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib_short",
            "{}",
            &counters([1819.0, 2600.0, 1.0, 2.0, 2675.0]),
        );
        assert_variant(
            &results,
            "fib_long",
            "{}",
            &counters([221_522.0, 315_862.0, 6.0, 4.0, 316_032.0]),
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_iai_v1_ignores_average() {
        for suffix in ["rust/iai/cache_sim.txt", "rust/iai/cache_sim_change.txt"] {
            assert_ignores_average(Adapter::RustIai, suffix);
        }
    }

    /// Kills a leaf that ingests the change against the last run in place of the count.
    #[test]
    fn adapter_rust_iai_v1_change() {
        let results = convert_v1(Adapter::RustIai, "rust/iai/cache_sim_change.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib_short",
            "{}",
            &counters([1777.0, 2538.0, 1.0, 2.0, 2613.0]),
        );
        assert_variant(
            &results,
            "fib_long",
            "{}",
            &counters([221_480.0, 315_800.0, 7.0, 3.0, 315_940.0]),
        );
    }
}
