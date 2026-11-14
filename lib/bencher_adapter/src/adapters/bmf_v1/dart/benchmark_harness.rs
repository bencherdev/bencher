use std::collections::HashSet;

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Cycles, Latency, PageFaults},
        },
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::{bmf_v1::leaf::LeafResults, util::Units},
    results::adapter_results::AdapterResults,
};

/// Dart's `benchmark_harness` at version 1: the lines of its `PrintEmitter` and `PrintEmitterV2`,
/// each score a mean per exercise of the benchmark.
///
/// `MajorPageFaults` is perf's `page-faults` event, which counts every page fault. Under
/// `benchmark_harness:bench` each run carries `@flavor`, the runtime its `--flavor` names; adapter
/// keys start with `@`, following JSON-LD's convention for keywords.
pub struct AdapterDartBenchmarkHarnessV1;

impl Adaptable for AdapterDartBenchmarkHarnessV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let (mut flavor, mut parameters) = (None, ParameterSet::default());
        let mut seen = HashSet::new();
        for line in input.lines() {
            if let Some((_, name)) = FLAVORS.iter().find(|(header, _)| *header == line) {
                flavor = Some(*name);
                parameters = flavor_parameters(name).unwrap_or_default();
                continue;
            }
            let Some(Score {
                name,
                metric,
                value,
                unit,
            }) = parse_line(line)
            else {
                continue;
            };
            if metric == "Count" && unit.is_empty() && name.ends_with(".totalIterations") {
                continue;
            }
            let Some((measure, factor)) = measure_of(metric, unit) else {
                results.skip_measure();
                continue;
            };
            let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            if !seen.insert((name, flavor, metric)) {
                results.skip_measure();
                continue;
            }
            let mut variant = results.variant(benchmark_name, parameters.clone());
            if value.is_finite() {
                variant.insert(measure, "mean", value * factor);
            } else {
                variant.skip_metric();
            }
        }
        results.finish(Adapter::DartBenchmarkHarness)
    }
}

/// The header `benchmark_harness:bench` prints before it runs a flavor, and the flavor's name.
const FLAVORS: [(&str, &str); 4] = [
    ("JIT - RUN", "jit"),
    ("AOT - RUN", "aot"),
    ("JS - RUN", "js"),
    ("WASM - RUN", "wasm"),
];

fn flavor_parameters(flavor: &str) -> Option<ParameterSet> {
    let mut parameters = serde_json::Map::new();
    parameters.insert("@flavor".to_owned(), flavor.into());
    serde_json::from_value(serde_json::Value::Object(parameters)).ok()
}

/// One emitted score.
struct Score<'a> {
    name: &'a str,
    metric: &'a str,
    value: f64,
    unit: &'a str,
}

/// `Name(Metric): value` with an optional unit after a space, where the value is a Dart double.
fn parse_line(line: &str) -> Option<Score<'_>> {
    let (head, score) = line.rsplit_once("): ")?;
    let (name, metric) = head.rsplit_once('(')?;
    let (value, unit) = score.split_once(' ').unwrap_or((score, ""));
    Some(Score {
        name: name.trim(),
        metric,
        value: value.parse().ok()?,
        unit,
    })
}

/// The measure of a metric and the factor to its unit, or `None` for a metric this leaf does not
/// know.
fn measure_of(metric: &str, unit: &str) -> Option<(MeasureNameId, f64)> {
    match (metric, unit) {
        ("RunTime", "us." | "") => Some((Latency::name_id(), Units::Micro.as_nanos())),
        ("CpuCycles", "") => Some((Cycles::name_id(), 1.0)),
        ("MajorPageFaults", "") => Some((PageFaults::name_id(), 1.0)),
        _ => None,
    }
}

#[cfg(test)]
mod test_dart_benchmark_harness_v1 {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Cycles, Latency, PageFaults},
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_v1,
        },
        results::adapter_results::Skipped,
    };

    fn latency(nanos: f64) -> Expected {
        (Latency::SLUG_STR, vec![("mean", nanos)])
    }

    /// Kills `RunTime` left in microseconds or stored as `value`, `PrintEmitterV2`'s score with no
    /// unit refused, perf's counts dropped or put in the wrong measure, the iteration count
    /// stored or counted as skipped, a name cut at its first `(`, and a leaf that records no leaf.
    #[test]
    fn adapter_dart_benchmark_harness_v1_emitters() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/emitters.txt",
        );
        assert_eq!(
            results.resolved_adapter,
            Some(Adapter::DartBenchmarkHarness)
        );
        assert!(results.skipped.is_empty());
        assert_eq!(results.inner.len(), 5);
        for (name, expected) in [
            ("Template", latency(3.467_445_333_333_333)),
            ("List.filled(100)", latency(557.751_820_247_679)),
            ("TemplateV2", latency(5.592_896_988_814_206)),
            ("Delayed", latency(255.300_256_717_173_17)),
        ] {
            assert_variant(&results, name, "{}", &[expected]);
        }
        assert_variant(
            &results,
            "Perf",
            "{}",
            &[
                latency(362.601_166_666_666_64),
                (Cycles::SLUG_STR, vec![("mean", 1_879.243_654_920_285)]),
                (
                    PageFaults::SLUG_STR,
                    vec![("mean", 9.268_144_517_467_603e-5)],
                ),
            ],
        );
    }

    /// Kills a run of `benchmark_harness:bench` without `@flavor`, which merges every flavor's
    /// runs of a benchmark into one variant, or a flavor's key spelled without `@`.
    #[test]
    fn adapter_dart_benchmark_harness_v1_flavors() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/bench.txt",
        );
        assert!(results.skipped.is_empty());
        for (flavor, template, list) in [
            ("jit", 3.471_176, 510.836_25),
            ("aot", 5.788_705_994_211_294, 413.816_000_576_801_97),
            ("js", 9.437_999_959_888_5, 595.25),
        ] {
            let parameters = format!(r#"{{"@flavor":"{flavor}"}}"#);
            assert_variant(&results, "Template", &parameters, &[latency(template)]);
            assert_variant(&results, "List.filled(100)", &parameters, &[latency(list)]);
        }
    }

    /// Kills a metric the leaf does not know stored or left uncounted, and a benchmark reported
    /// twice that overwrites its first score or goes uncounted.
    #[test]
    fn adapter_dart_benchmark_harness_v1_custom() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/custom.txt",
        );
        assert_eq!(results.skipped.measures, 2);
        assert_eq!(results.inner.len(), 1);
        assert_variant(&results, "Template", "{}", &[latency(3.286_688)]);
    }

    /// Kills a user's own `Count` score, or a score of a name that ends in `.totalIterations` but
    /// is not the iteration count `PerfBenchmarkBase` writes, dropped uncounted like that count.
    #[test]
    fn adapter_dart_benchmark_harness_v1_counts() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/counted.txt",
        );
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 2,
                ..Skipped::default()
            }
        );
        assert_eq!(results.inner.len(), 2);
        assert_variant(&results, "MyBench", "{}", &[latency(597.753_25)]);
        assert_variant(
            &results,
            "Loop.totalIterations",
            "{}",
            &[latency(291.097_410_501_652_3)],
        );
    }

    /// Kills a custom `PrintEmitterV2` metric whose name holds a character other than a letter or
    /// a digit (`GC_Count`, `Heap Size`, `Heap (MB)`) dropped uncounted, and a known metric in a
    /// unit the leaf does not know stored.
    #[test]
    fn adapter_dart_benchmark_harness_v1_custom_metrics() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/metrics.txt",
        );
        assert_eq!(results.skipped.measures, 5);
        assert_variant(
            &results,
            "WithCustom",
            "{}",
            &[latency(290.722_397_853_044_13)],
        );
    }

    /// Kills a score that is not finite (`NaN`, `Infinity`) stored or left uncounted.
    #[test]
    fn adapter_dart_benchmark_harness_v1_not_finite() {
        let results = convert_v1(
            Adapter::DartBenchmarkHarness,
            "dart/benchmark_harness/metrics.txt",
        );
        assert_eq!(results.skipped.metrics, 2);
        for name in ["Unmeasured", "Unbounded"] {
            let latency = results
                .get(name)
                .and_then(|metrics| metrics.inner.get(&Latency::name_id()));
            assert!(latency.is_none(), "{name}");
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_dart_benchmark_harness_v1_ignores_average() {
        for suffix in [
            "dart/benchmark_harness/bench.txt",
            "dart/benchmark_harness/custom.txt",
            "dart/benchmark_harness/emitters.txt",
            "dart/benchmark_harness/metrics.txt",
            "dart/benchmark_harness/one.txt",
            "dart/benchmark_harness/two.txt",
        ] {
            assert_ignores_average(Adapter::DartBenchmarkHarness, suffix);
        }
    }
}
