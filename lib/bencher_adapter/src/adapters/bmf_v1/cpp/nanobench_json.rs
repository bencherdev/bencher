use bencher_json::project::{
    measure::built_in::{
        BuiltInMeasure as _,
        default::{BranchMisses, Cycles, Latency, PageFaults},
        iai::Instructions,
        mitata::Branches,
        systems::ContextSwitches,
    },
    report::Adapter,
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::nanobench::{insert_rate, variant},
        finite::Finite,
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// nanobench's `templates::json()` rendered once over every result, each time and counter per
/// unit, as nanobench's table divides them by `batch`.
pub struct AdapterCppNanobenchJson;

impl Adaptable for AdapterCppNanobenchJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Report { results: runs } = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for run in &runs {
            run.insert(&mut results);
        }
        results.finish_with_skips(Adapter::CppNanobenchJson)
    }
}

#[derive(Deserialize)]
struct Report {
    results: Vec<Run>,
}

/// One result, its times in seconds and its counters per iteration.
#[derive(Deserialize)]
struct Run {
    title: String,
    name: String,
    unit: String,
    batch: f64,
    #[serde(rename = "complexityN")]
    complexity_n: f64,
    #[serde(rename = "median(elapsed)")]
    elapsed: f64,
    #[serde(rename = "median(instructions)")]
    instructions: f64,
    #[serde(rename = "median(cpucycles)")]
    cycles: f64,
    #[serde(rename = "median(contextswitches)")]
    context_switches: f64,
    #[serde(rename = "median(pagefaults)")]
    page_faults: f64,
    #[serde(rename = "median(branchinstructions)")]
    branches: f64,
    #[serde(rename = "median(branchmisses)")]
    branch_misses: f64,
    measurements: Vec<Measurement>,
}

impl Run {
    fn insert(&self, results: &mut LeafResults) {
        let Self {
            title,
            name,
            unit,
            batch,
            complexity_n,
            elapsed: median,
            instructions,
            cycles,
            context_switches,
            page_faults,
            branches,
            branch_misses,
            measurements,
        } = self;
        // An overflowed benchmark has no measurements and a median of 0.
        if measurements.is_empty() {
            results.skip_benchmark();
            return;
        }
        let Some(mut variant) = variant(results, title, name, Some(*complexity_n)) else {
            return;
        };
        let per_unit = |value: f64| Finite::from(Some(value / batch));
        let latency = Latency::name_id();
        let elapsed = measurements.iter().map(|measurement| measurement.elapsed);
        // `Result::minimum`, `maximum`, and `average` (`nanobench.h` v4.3.11 :3060-3078, :2993-3002).
        let min = elapsed.clone().fold(f64::INFINITY, f64::min);
        let max = elapsed.clone().fold(f64::NEG_INFINITY, f64::max);
        let (sum, epochs) = elapsed.fold((0.0, 0.0), |(sum, epochs), seconds| {
            (sum + seconds, epochs + 1.0)
        });
        let mean = sum / epochs;
        for (metric, seconds) in [
            ("median", *median),
            ("min", min),
            ("max", max),
            ("mean", mean),
        ] {
            per_unit(seconds).insert(&mut variant, &latency, metric, NANOSECONDS_PER_SECOND);
        }
        insert_rate(&mut variant, unit, Finite::from(Some(batch / median)));
        for (measure, count) in [
            (Instructions::name_id(), instructions),
            (Cycles::name_id(), cycles),
            (Branches::name_id(), branches),
            (BranchMisses::name_id(), branch_misses),
            (PageFaults::name_id(), page_faults),
            (ContextSwitches::name_id(), context_switches),
        ] {
            per_unit(*count).insert(&mut variant, &measure, "median", 1.0);
        }
    }
}

const NANOSECONDS_PER_SECOND: f64 = 1e9;

/// One epoch, its time per iteration in seconds.
#[derive(Deserialize)]
struct Measurement {
    elapsed: f64,
}

#[cfg(test)]
mod test_cpp_nanobench_json {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BranchMisses, BytesThroughput, Cycles, Latency, PageFaults, Throughput},
            iai::Instructions,
            mitata::Branches,
            systems::ContextSwitches,
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{
        assert_ignores_average, assert_value, assert_variant, convert_v1, variants,
    };

    /// Kills a time or a counter per iteration instead of per unit, a `min`, `max`, or `mean`
    /// dropped or not per unit, a `mean` weighted by each epoch's iterations, a byte rate in
    /// `throughput`, a counter in another counter's measure, and nanobench's default title
    /// written as `@title`.
    #[test]
    fn adapter_cpp_nanobench_json_bitcoin() {
        let results = convert_v1(Adapter::CppNanobenchJson, "cpp/nanobench/bitcoin.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::CppNanobenchJson));
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 10);
        assert_variant(
            &results,
            "Base58Decode",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("median", 12.097_516_278_165_735),
                        ("min", 11.398_053_505_164_295),
                        ("max", 14.449_841_737_440_85),
                        ("mean", 12.415_710_990_307_71),
                    ],
                ),
                (
                    BytesThroughput::SLUG_STR,
                    vec![("median", 82_661_595.736_378_98)],
                ),
                (
                    Instructions::SLUG_STR,
                    vec![("median", 284.559_050_472_040_6)],
                ),
                (Cycles::SLUG_STR, vec![("median", 64.660_345_138_763_53)]),
                (Branches::SLUG_STR, vec![("median", 38.882_411_297_852_35)]),
                (
                    BranchMisses::SLUG_STR,
                    vec![("median", 0.004_093_358_156_509_294)],
                ),
                (PageFaults::SLUG_STR, vec![("median", 0.0)]),
                (ContextSwitches::SLUG_STR, vec![("median", 0.0)]),
            ],
        );
        assert_value(
            &results,
            ("AddrManAdd", "{}"),
            (Throughput::SLUG_STR, "median"),
            60.281_272_417_098_18,
        );
        assert_value(
            &results,
            (
                "SHA256_32b_AVX2 using the 'sse4(1way);sse41(4way);avx2(8way)' SHA256 \
                 implementation",
                "{}",
            ),
            (Latency::SLUG_STR, "median"),
            4.035_414_648_491_563,
        );
    }

    /// Kills a title the user set dropped or put in the name, a `complexityN` the user set
    /// dropped, nanobench's unset `-1` written, and the zeros of a run without perf counters
    /// dropped or counted as skipped.
    #[test]
    fn adapter_cpp_nanobench_json_title_and_complexity() {
        let results = convert_v1(Adapter::CppNanobenchJson, "cpp/nanobench/json.json");
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 3);
        assert_value(
            &results,
            ("std::set insert", r#"{"@title":"Containers"}"#),
            (Latency::SLUG_STR, "median"),
            91.981_948_444_194_7,
        );
        assert_value(
            &results,
            ("std::vector push_back x1000", r#"{"@title":"Containers"}"#),
            (Throughput::SLUG_STR, "median"),
            2_460_436_751.457_593_4,
        );
        for measure in [
            Instructions::SLUG_STR,
            Cycles::SLUG_STR,
            Branches::SLUG_STR,
            BranchMisses::SLUG_STR,
            PageFaults::SLUG_STR,
            ContextSwitches::SLUG_STR,
        ] {
            assert_value(
                &results,
                (
                    "noisy | name with spaces",
                    r#"{"@complexityN":100,"@title":"Containers"}"#,
                ),
                (measure, "median"),
                0.0,
            );
        }
    }

    /// Kills `median(pagefaults)` and `median(contextswitches)` read into each other's measure,
    /// and page faults not divided by `batch`.
    #[test]
    fn adapter_cpp_nanobench_json_page_faults() {
        for (suffix, (name, parameters), page_faults) in [
            (
                "cpp/nanobench/json_perf_counters.json",
                ("std::set insert", r#"{"@title":"Containers"}"#),
                0.011_729_769_858_945_8,
            ),
            (
                "cpp/nanobench/bitcoin_sanity.json",
                ("Base58Encode", "{}"),
                1.0 / 32.0,
            ),
        ] {
            let results = convert_v1(Adapter::CppNanobenchJson, suffix);
            assert_value(
                &results,
                (name, parameters),
                (PageFaults::SLUG_STR, "median"),
                page_faults,
            );
            assert_value(
                &results,
                (name, parameters),
                (ContextSwitches::SLUG_STR, "median"),
                0.0,
            );
        }
    }

    /// Kills the result of an overflowed benchmark, which has no measurements, written with its
    /// median of 0 or skipped uncounted.
    #[test]
    fn adapter_cpp_nanobench_json_no_measurements() {
        let results = convert_v1(
            Adapter::CppNanobenchJson,
            "cpp/nanobench/json_4_6_0_overflow.json",
        );
        assert_eq!(variants(&results), 2);
        assert_eq!(results.skipped.benchmarks, 1);
        assert!(
            results
                .inner
                .keys()
                .all(|name| name.to_string() != "empty body")
        );
    }

    /// Kills a repeated name and title, as two `context()` runs print, that overwrites the first
    /// result or goes uncounted.
    #[test]
    fn adapter_cpp_nanobench_json_repeated_name() {
        let results = convert_v1(
            Adapter::CppNanobenchJson,
            "cpp/nanobench/json_4_6_0_context.json",
        );
        assert_eq!(variants(&results), 1);
        assert_eq!(results.skipped.variants, 1);
        assert_value(
            &results,
            ("vector fill", r#"{"@title":"Context"}"#),
            (Latency::SLUG_STR, "median"),
            6.452_351_898_049_351,
        );
    }

    /// Kills a result whose every measurement is the only one, as a sanity check run writes,
    /// read with a `mean` or a `min` that is not that measurement.
    #[test]
    fn adapter_cpp_nanobench_json_one_epoch() {
        let results = convert_v1(
            Adapter::CppNanobenchJson,
            "cpp/nanobench/bitcoin_sanity.json",
        );
        for metric in ["median", "min", "max", "mean"] {
            assert_value(
                &results,
                ("Base58Encode", "{}"),
                (Latency::SLUG_STR, metric),
                282.312_5,
            );
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_cpp_nanobench_json_ignores_average() {
        for suffix in ["cpp/nanobench/bitcoin.json", "cpp/nanobench/json.json"] {
            assert_ignores_average(Adapter::CppNanobenchJson, suffix);
        }
    }
}
