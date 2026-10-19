use bencher_json::{
    MeasureNameId,
    project::{
        measure::built_in::{BuiltInMeasure as _, default::Latency, gungraun::TotalBytes},
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            leaf::{LeafResults, Variant},
            rust::gungraun::tool_variant,
        },
        rust::gungraun_stdout::parse_stdout,
        util::Units,
    },
    results::adapter_results::{AdapterResults, GungraunMeasure, PerfStatistic},
};

/// gungraun's terminal output, each benchmark named by its whole first line, and each
/// tool's section its own variant.
pub struct AdapterRustGungraunStdoutV1;

impl Adaptable for AdapterRustGungraunStdoutV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for (benchmark_name, sections) in parse_stdout(input)? {
            for (tool, measures) in sections {
                let Some(mut variant) =
                    tool_variant(&mut results, benchmark_name.clone(), None, tool)
                else {
                    continue;
                };
                for measure in measures {
                    insert_measure(&mut variant, measure);
                }
            }
        }
        results.finish(Adapter::RustGungraunStdout)
    }
}

/// A counter is an exact count, so `value`, and a sampled perf event is the mean
/// of its samples. The unit of the time spent in system calls is an option the
/// output does not state, so that counter is skipped.
pub(super) fn insert_measure(variant: &mut Variant<'_>, measure: GungraunMeasure) {
    if let GungraunMeasure::Perf(label, metric, statistic) = measure {
        let statistic = match statistic {
            PerfStatistic::Value => "value",
            PerfStatistic::Mean => "mean",
        };
        if let Some((measure, value)) = perf_measure(variant, &label, metric.value.into_inner()) {
            variant.insert(measure, statistic, value);
        }
    } else if matches!(measure, GungraunMeasure::TimeSystemCalls(_)) {
        variant.skip_measure();
    } else if let Some((measure, metric)) = measure.into_measure() {
        variant.insert(measure, "value", metric.value.into_inner());
    } else {
        variant.skip_measure();
    }
}

/// A perf event, labeled as gungraun labels it, `task-clock [ms]` when it has a unit.
/// A time is in nanoseconds and a size in bytes, under the event's name; an event
/// in any other unit keeps its whole label, so a unit never mixes with another, and a
/// count with no unit counts its event.
pub(super) fn perf_measure(
    variant: &mut Variant<'_>,
    label: &str,
    value: f64,
) -> Option<(MeasureNameId, f64)> {
    let Some((event, unit)) = label
        .strip_suffix(']')
        .and_then(|label| label.rsplit_once(" ["))
    else {
        return Some((variant.named(label, label)?, value));
    };
    if let Some((factor, units)) = scale(unit) {
        return Some((variant.named(event, units)?, value * factor));
    }
    Some((variant.named(label, unit)?, value))
}

fn scale(unit: &str) -> Option<(f64, &'static str)> {
    if let Ok(units) = unit.parse::<Units>() {
        return Some((units.as_nanos(), Latency::UNITS_STR));
    }
    let factor = match unit {
        "B" => 1.0,
        "KB" => KILO,
        "MB" => KILO * KILO,
        "GB" => KILO * KILO * KILO,
        "KiB" => KIBI,
        "MiB" => KIBI * KIBI,
        "GiB" => KIBI * KIBI * KIBI,
        _ => return None,
    };
    Some((factor, TotalBytes::UNITS_STR))
}

const KILO: f64 = 1_000.0;
const KIBI: f64 = 1_024.0;

#[cfg(test)]
mod test_rust_gungraun_stdout_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency, gungraun},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_v1, variants,
        },
    };

    use super::AdapterRustGungraunStdoutV1;

    fn callgrind(values: [f64; 6]) -> Vec<Expected> {
        [
            gungraun::Instructions::SLUG_STR,
            gungraun::L1Hits::SLUG_STR,
            gungraun::LLHits::SLUG_STR,
            gungraun::RamHits::SLUG_STR,
            gungraun::TotalReadWrite::SLUG_STR,
            gungraun::EstimatedCycles::SLUG_STR,
        ]
        .into_iter()
        .zip(values)
        .map(|(measure, value)| (measure, vec![("value", value)]))
        .collect()
    }

    fn errors(measures: [&'static str; 4]) -> Vec<Expected> {
        measures
            .into_iter()
            .map(|measure| (measure, vec![("value", 0.0)]))
            .collect()
    }

    /// Ruling 55: gungraun prints callgrind's counters with no tool header only when
    /// callgrind runs alone, so a section with no header is callgrind's.
    ///
    /// Kills a leaf that drops the first line's arguments from the name, leaves out the
    /// tool of a section with no header, reads the previous run, or records no leaf.
    #[test]
    fn adapter_rust_gungraun_stdout_v1() {
        for adapter in [Adapter::RustGungraun, Adapter::RustGungraunStdout] {
            let results = convert_v1(adapter, "rust/gungraun/stdout_v020.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustGungraunStdout),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 5, "{adapter}");
            assert_eq!(variants(&results), 5, "{adapter}");
            assert_variant(
                &results,
                "gung::fib_group::bench_fib short:(10)",
                r#"{"@tool": "callgrind"}"#,
                &callgrind([1802.0, 2572.0, 1.0, 2.0, 2575.0, 2647.0]),
            );
            // Issue 811: the output truncates a long description, and the name keeps it as printed.
            assert_variant(
                &results,
                r#"gung::fib_group::bench_text long_argument:("a deliberately long argument that the descriptio..."#,
                r#"{"@tool": "callgrind"}"#,
                &callgrind([412.0, 450.0, 0.0, 8.0, 458.0, 730.0]),
            );
        }
    }

    /// Ruling 53: each tool section the output heads is its own variant, so
    /// callgrind's and cachegrind's shared counters both stand.
    ///
    /// Kills a leaf that lets one tool overwrite another, or drops a tool's section.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_tools() {
        let results = convert_v1(
            Adapter::RustGungraunStdout,
            "rust/gungraun/stdout_v020_tools.txt",
        );
        let benchmark = "gung::fib_group::bench_fib short:(10)";
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "callgrind"}"#,
            &callgrind([1802.0, 2572.0, 1.0, 2.0, 2575.0, 2647.0]),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "cachegrind"}"#,
            &callgrind([397_307.0, 535_299.0, 948.0, 4577.0, 540_824.0, 700_234.0]),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "dhat"}"#,
            &[
                gungraun::TotalBytes::SLUG_STR,
                gungraun::TotalBlocks::SLUG_STR,
                gungraun::AtTGmaxBytes::SLUG_STR,
                gungraun::AtTGmaxBlocks::SLUG_STR,
                gungraun::AtTEndBytes::SLUG_STR,
                gungraun::AtTEndBlocks::SLUG_STR,
                gungraun::ReadsBytes::SLUG_STR,
                gungraun::WritesBytes::SLUG_STR,
            ]
            .map(|measure| (measure, vec![("value", 0.0)])),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "memcheck"}"#,
            &errors([
                gungraun::MemcheckErrors::SLUG_STR,
                gungraun::MemcheckContexts::SLUG_STR,
                gungraun::MemcheckSuppressedErrors::SLUG_STR,
                gungraun::MemcheckSuppressedContexts::SLUG_STR,
            ]),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "helgrind"}"#,
            &errors([
                gungraun::HelgrindErrors::SLUG_STR,
                gungraun::HelgrindContexts::SLUG_STR,
                gungraun::HelgrindSuppressedErrors::SLUG_STR,
                gungraun::HelgrindSuppressedContexts::SLUG_STR,
            ]),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "drd"}"#,
            &errors([
                gungraun::DrdErrors::SLUG_STR,
                gungraun::DrdContexts::SLUG_STR,
                gungraun::DrdSuppressedErrors::SLUG_STR,
                gungraun::DrdSuppressedContexts::SLUG_STR,
            ]),
        );
        assert_eq!(variants(&results), 30);
    }

    /// Kills a leaf that drops perf events, keeps a time in milliseconds, names an
    /// event with its unit, or creates an event in the generic unit.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_perf() {
        let results = convert_v1(
            Adapter::RustGungraunStdout,
            "rust/gungraun/stdout_v020_perf.txt",
        );
        let benchmark = "gung::fib_group::bench_fib short:(10)";
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "callgrind"}"#,
            &callgrind([1802.0, 2572.0, 1.0, 2.0, 2575.0, 2647.0]),
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@tool": "perf"}"#,
            &[
                ("instructions/u", vec![("value", 3707.0)]),
                ("cycles/u", vec![("value", 23122.0)]),
                ("task-clock", vec![("value", 5_591_530.0)]),
                ("cpu-clock", vec![("value", 5_590_140.0)]),
                ("faults", vec![("value", 5.0)]),
                ("context-switches", vec![("value", 0.0)]),
                ("branch-misses", vec![("value", 908.0)]),
                ("cache-misses", vec![("value", 5111.0)]),
            ],
        );
        assert_units(
            &results,
            &[
                ("instructions/u", "instructions/u"),
                ("cycles/u", "cycles/u"),
                ("task-clock", Latency::UNITS_STR),
                ("cpu-clock", Latency::UNITS_STR),
                ("faults", "faults"),
                ("context-switches", "context-switches"),
                ("branch-misses", "branch-misses"),
                ("cache-misses", "cache-misses"),
            ],
        );
    }

    /// Kills a leaf that names a sampled event's mean `value`, or drops the events
    /// after the first when gungraun prints sample lines under each.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_perf_sampling() {
        let results = convert_v1(
            Adapter::RustGungraunStdout,
            "rust/gungraun/stdout_v020_perf_sampling.txt",
        );
        assert_eq!(results.inner.len(), 5);
        assert_variant(
            &results,
            "gung::fib_group::bench_fib short:(10)",
            r#"{"@tool": "perf"}"#,
            &[
                ("instructions/u", vec![("mean", 724.0)]),
                ("cycles/u", vec![("mean", 6604.0)]),
                ("task-clock", vec![("mean", 56_469.2)]),
                ("cpu-clock", vec![("mean", 117_164.0)]),
                ("faults", vec![("mean", 1.0)]),
                ("context-switches", vec![("mean", 0.0)]),
                ("branch-misses", vec![("mean", 221.0)]),
                ("cache-misses", vec![("mean", 592.0)]),
            ],
        );
    }

    /// Kills a leaf that keeps the time in system calls or skips it uncounted.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_systime() {
        let results = convert_v1(
            Adapter::RustGungraunStdout,
            "rust/gungraun/stdout_v020_systime.txt",
        );
        assert_eq!(results.inner.len(), 5);
        assert_eq!(results.skipped.measures, 5);
        let metrics = results
            .inner
            .values()
            .flat_map(|variants| variants.values())
            .next()
            .expect("a benchmark");
        assert!(metrics.inner.contains_key(&gungraun::SysCpuTime::name_id()));
        assert!(!metrics.inner.contains_key(&gungraun::SysTime::name_id()));
    }

    /// Kills a leaf that skips a counter it does not know without counting it.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_unknown_counter() {
        let results = AdapterRustGungraunStdoutV1::parse(
            "gung::group::bench_fib\n  Instructions:  1802|N/A  (*********)\n  Bogus Counter:  5|N/A  (*********)\n",
            Settings::default(),
        )
        .expect("Failed to parse");
        assert_variant(
            &results,
            "gung::group::bench_fib",
            r#"{"@tool": "callgrind"}"#,
            &[(gungraun::Instructions::SLUG_STR, vec![("value", 1802.0)])],
        );
        assert_eq!(results.skipped.measures, 1);
    }

    /// Kills a node that tries the text leaf before the JSON leaf.
    #[test]
    fn adapter_rust_gungraun_v1_tries_json_first() {
        let input = ["json_v7_callgrind.txt", "stdout_v020.txt"]
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/rust/gungraun/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n");
        let results = Adapter::RustGungraun
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustGungraunJson));
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_gungraun_stdout_v1_ignores_average() {
        for suffix in [
            "rust/gungraun/stdout_v020_tools.txt",
            "rust/gungraun/stdout_v020_perf.txt",
        ] {
            assert_ignores_average(Adapter::RustGungraunStdout, suffix);
        }
    }
}
