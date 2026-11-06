use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, JsonNewMetric,
    project::{
        measure::built_in::{BuiltInMeasure as _, gungraun::ProfileTime},
        report::Adapter,
    },
};
use gungraun_runner::api::Tool;
use gungraun_summary::{
    either_or_both::EitherOrBoth,
    util::SummaryByVersion,
    v6::{self, ValgrindTool},
    v7::{self, Metric, PerfStats, StatisticalMetric, ToolMetricResults},
};
use rust_decimal::{Decimal, prelude::ToPrimitive as _};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            leaf::{LeafResults, Variant},
            rust::{
                gungraun::tool_variant,
                gungraun_stdout::{insert_measure, perf_measure},
            },
        },
        rust::gungraun_json::{
            cachegrind_measure, callgrind_measure, dhat_measure, drd_measure, helgrind_measure,
            memcheck_measure, parse_blocks, profile_measures,
        },
        util::{Units, latency_as_nanos},
    },
    results::adapter_results::{AdapterResults, GungraunMeasure, GungraunTool},
};

/// gungraun's summaries, schema 6 and schema 7.
///
/// A benchmark is named by `module_path` with its `@id` and each profile's `@tool` as
/// parameters, since gungraun documents its `description` as unstable. Only the totals
/// of this run are read: the previous run and the regressions are local comparisons.
pub struct AdapterRustGungraunJsonV1;

impl Adaptable for AdapterRustGungraunJsonV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let input = strip_ansi_escapes::strip_str(input);
        let summaries = parse_blocks(&input, |block| {
            match gungraun_summary::util::parse_slice(block.as_bytes()) {
                Ok(SummaryByVersion::V6(summary)) => Some(Summary::V6(Box::new(summary))),
                Ok(SummaryByVersion::V7(summary)) => Some(Summary::V7(Box::new(summary))),
                Ok(_) | Err(_) => None,
            }
        })?;

        let mut results = LeafResults::default();
        for summary in summaries {
            match summary {
                Summary::V6(summary) => insert_v6(&mut results, *summary),
                Summary::V7(summary) => insert_v7(&mut results, *summary),
            }
        }
        results.finish(Adapter::RustGungraunJson)
    }
}

fn insert_v6(results: &mut LeafResults, summary: v6::BenchmarkSummary) {
    let v6::BenchmarkSummary {
        module_path,
        id,
        profiles,
        ..
    } = summary;
    let Some(benchmark_name) = benchmark_name(results, &module_path) else {
        return;
    };
    for profile in profiles.0 {
        let tool = v6_tool(profile.tool);
        let Some(mut variant) = tool_variant(results, benchmark_name.clone(), id.as_deref(), tool)
        else {
            continue;
        };
        for measure in profile_measures(profile) {
            insert_measure(&mut variant, measure);
        }
    }
}

fn insert_v7(results: &mut LeafResults, summary: v7::BenchmarkSummary) {
    let v7::BenchmarkSummary {
        module_path,
        id,
        profiles,
        ..
    } = summary;
    let Some(benchmark_name) = benchmark_name(results, &module_path) else {
        return;
    };
    for profile in profiles.0 {
        let v7::Profile {
            data,
            process_ns,
            tool,
            ..
        } = profile;
        let Some(mut variant) = tool_variant(
            results,
            benchmark_name.clone(),
            id.as_deref(),
            v7_tool(tool),
        ) else {
            continue;
        };
        variant.insert(
            ProfileTime::name_id(),
            "value",
            latency_as_nanos(process_ns, Units::Nano).into_inner(),
        );
        for measure in total(data.total.metrics) {
            insert_measure(&mut variant, measure);
        }
        insert_perf(&mut variant, data.parts);
    }
}

/// A benchmark named by its module path, or `None`, counted as skipped, if the path is
/// not a valid name.
fn benchmark_name(results: &mut LeafResults, module_path: &str) -> Option<BenchmarkName> {
    let benchmark_name = module_path.parse().ok();
    if benchmark_name.is_none() {
        results.skip_benchmark();
    }
    benchmark_name
}

fn v6_tool(tool: ValgrindTool) -> GungraunTool {
    match tool {
        ValgrindTool::Callgrind => GungraunTool::Callgrind,
        ValgrindTool::Cachegrind => GungraunTool::Cachegrind,
        ValgrindTool::DHAT => GungraunTool::Dhat,
        ValgrindTool::Memcheck => GungraunTool::Memcheck,
        ValgrindTool::Helgrind => GungraunTool::Helgrind,
        ValgrindTool::DRD => GungraunTool::Drd,
        ValgrindTool::Massif => GungraunTool::Massif,
        ValgrindTool::BBV => GungraunTool::Bbv,
    }
}

fn v7_tool(tool: Tool) -> GungraunTool {
    match tool {
        Tool::Callgrind => GungraunTool::Callgrind,
        Tool::Cachegrind => GungraunTool::Cachegrind,
        Tool::DHAT => GungraunTool::Dhat,
        Tool::Memcheck => GungraunTool::Memcheck,
        Tool::Helgrind => GungraunTool::Helgrind,
        Tool::DRD => GungraunTool::Drd,
        Tool::Massif => GungraunTool::Massif,
        Tool::BBV => GungraunTool::Bbv,
        Tool::Perf => GungraunTool::Perf,
    }
}

/// Every counter in a profile's total.
fn total(metrics: ToolMetricResults) -> Vec<GungraunMeasure> {
    match metrics {
        ToolMetricResults::Callgrind(metrics) => new_values(metrics.0, callgrind_measure),
        ToolMetricResults::Cachegrind(metrics) => new_values(metrics.0, cachegrind_measure),
        ToolMetricResults::Dhat(metrics) => new_values(metrics.0, dhat_measure),
        ToolMetricResults::Memcheck(metrics) => new_values(metrics.0, memcheck_measure),
        ToolMetricResults::Helgrind(metrics) => new_values(metrics.0, helgrind_measure),
        ToolMetricResults::DRD(metrics) => new_values(metrics.0, drd_measure),
        // A perf profile's total holds no metric, only its regressions.
        ToolMetricResults::Perf(_) | ToolMetricResults::None => Vec::new(),
    }
}

fn new_values<K>(
    metrics: impl IntoIterator<Item = (K, v7::MetricResult)>,
    measure: fn(K, JsonNewMetric) -> GungraunMeasure,
) -> Vec<GungraunMeasure> {
    metrics
        .into_iter()
        .filter_map(|(kind, result)| {
            let (EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _)) = result.values else {
                return None;
            };
            Some(measure(
                kind,
                JsonNewMetric {
                    value: metric_value(new).into(),
                    lower_value: None,
                    upper_value: None,
                },
            ))
        })
        .collect()
}

/// Each perf event of a profile's parts, with the mean gungraun reports when it
/// repeats the event. The summary gives no total over parts, so an event in more
/// than one part is skipped.
fn insert_perf(variant: &mut Variant<'_>, parts: Vec<v7::ProfilePart>) {
    let mut events = BTreeMap::<String, Vec<StatisticalMetric<PerfStats>>>::new();
    for part in parts {
        let ToolMetricResults::Perf(metrics) = part.metrics else {
            continue;
        };
        for (event, result) in metrics.0 {
            let (EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _)) = result.values else {
                continue;
            };
            events.entry(event.0).or_default().push(new);
        }
    }
    for (event, mut metrics) in events {
        let Some(metric) = metrics.pop().filter(|_| metrics.is_empty()) else {
            variant.skip_measure();
            continue;
        };
        let StatisticalMetric { stats, unit, value } = metric;
        let label = if let Some(unit) = unit {
            format!("{event} [{unit}]")
        } else {
            event
        };
        let (statistic, value) = if let Some(mean) = stats.mean {
            ("mean", mean)
        } else {
            ("value", metric_value(value))
        };
        if let Some((measure, value)) = perf_measure(variant, &label, value) {
            variant.insert(measure, statistic, value);
        }
    }
}

fn metric_value(metric: Metric) -> f64 {
    match metric {
        Metric::Int(int) => Decimal::from(int).to_f64().unwrap_or_default(),
        Metric::Float(float) => float,
    }
}

enum Summary {
    V6(Box<v6::BenchmarkSummary>),
    V7(Box<v7::BenchmarkSummary>),
}

#[cfg(test)]
#[expect(
    clippy::unreadable_literal,
    reason = "counters as the harness wrote them"
)]
mod test_rust_gungraun_json_v1 {
    use bencher_json::{
        BenchmarkNameId,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency, gungraun},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::{
            bmf_v1::test_util::{
                Expected, assert_ignores_average, assert_units, assert_variant, convert_v1,
                variants,
            },
            json::v1::test_json_v1::{named, variant},
        },
    };

    use super::AdapterRustGungraunJsonV1;

    fn expected(metrics: &[(&'static str, &'static str, f64)]) -> Vec<Expected> {
        metrics
            .iter()
            .map(|(measure, metric, value)| (*measure, vec![(*metric, *value)]))
            .collect()
    }

    /// Kills a leaf that refuses schema 7, names a benchmark by anything but its
    /// module path, puts its `@id` in the name rather than a parameter (issues 778
    /// and 811), leaves out the tool, reads the previous run, drops the profile's
    /// time, or records no leaf.
    #[test]
    fn adapter_rust_gungraun_json_v7_callgrind() {
        for adapter in [Adapter::RustGungraun, Adapter::RustGungraunJson] {
            let results = convert_v1(adapter, "rust/gungraun/json_v7_callgrind.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustGungraunJson),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 4, "{adapter}");
            assert_variant(
                &results,
                "gung::fib_group::bench_fib",
                r#"{"@id": "short", "@tool": "callgrind"}"#,
                &expected(CALLGRIND_SHORT),
            );
            assert_variant(
                &results,
                "gung::fib_group::bench_no_id",
                r#"{"@tool": "callgrind"}"#,
                &expected(CALLGRIND_NO_ID),
            );
            assert_eq!(variants(&results), 5, "{adapter}");
        }
    }

    /// Ruling 53: each tool's run is its own variant, so callgrind's and cachegrind's
    /// shared counters both stand. Kills a leaf that lets one tool overwrite another,
    /// drops a tool, folds the error tools into one measure, or drops a profile's time.
    #[test]
    fn adapter_rust_gungraun_json_v7_tools() {
        let results = convert_v1(Adapter::RustGungraunJson, "rust/gungraun/json_v7_tools.txt");
        let benchmark = "gung::fib_group::bench_alloc";
        let tool = |tool: &str| format!(r#"{{"@id": "allocate", "@tool": "{tool}"}}"#);
        assert_variant(
            &results,
            benchmark,
            &tool("callgrind"),
            &expected(TOOLS_CALLGRIND),
        );
        assert_variant(
            &results,
            benchmark,
            &tool("cachegrind"),
            &expected(TOOLS_CACHEGRIND),
        );
        assert_variant(&results, benchmark, &tool("dhat"), &expected(TOOLS_DHAT));
        for (name, errors) in [
            ("memcheck", TOOLS_MEMCHECK),
            ("helgrind", TOOLS_HELGRIND),
            ("drd", TOOLS_DRD),
        ] {
            assert_variant(&results, benchmark, &tool(name), &expected(errors));
        }
        let benchmark = benchmark.parse::<BenchmarkNameId>().unwrap();
        assert_eq!(results.inner[&benchmark].len(), 6);
    }

    /// Kills a leaf that drops perf, reads its events from the empty total, leaves a
    /// time in milliseconds, or gives an event no unit, or a time any unit but
    /// nanoseconds.
    #[test]
    fn adapter_rust_gungraun_json_v7_perf() {
        let results = convert_v1(Adapter::RustGungraunJson, "rust/gungraun/json_v7_perf.txt");
        assert_variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "callgrind"}"#,
            &expected(PERF_SHORT_CALLGRIND),
        );
        assert_variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "perf"}"#,
            &expected(PERF_SHORT_PERF),
        );
        assert_units(&results, &perf_units());
    }

    /// Kills a leaf that reads a sampled event's rounded value in place of its
    /// mean, or keeps the mean in microseconds.
    #[test]
    fn adapter_rust_gungraun_json_v7_perf_sampling() {
        let results = convert_v1(
            Adapter::RustGungraunJson,
            "rust/gungraun/json_v7_perf_sampling.txt",
        );
        assert_variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "perf"}"#,
            &[
                (
                    gungraun::ProfileTime::SLUG_STR,
                    vec![("value", 602786859.0)],
                ),
                ("branch-misses", vec![("mean", 261.7301587301586)]),
                ("cache-misses", vec![("mean", 754.3620689655173)]),
                ("context-switches", vec![("mean", 0.0)]),
                ("cpu-clock", vec![("mean", 141761.42696629216)]),
                ("cycles:u", vec![("mean", 5456.349593495936)]),
                ("faults", vec![("mean", 1.2375000000000003)]),
                ("instructions:u", vec![("mean", 664.2520325203254)]),
                ("task-clock", vec![("mean", 64852.91588785043)]),
            ],
        );
        assert_units(&results, &perf_units());
    }

    /// Kills a leaf that keeps one part's count of an event more than one part
    /// reports, or skips it uncounted.
    #[test]
    fn adapter_rust_gungraun_json_v7_perf_parts() {
        let mut summary = perf_summary();
        let parts = summary["profiles"][1]["data"]["parts"]
            .as_array_mut()
            .expect("perf parts");
        let mut second = parts[0].clone();
        second["metrics"]
            .as_object_mut()
            .expect("perf metrics")
            .retain(|event, _| event == "faults");
        parts.push(second);

        let results = AdapterRustGungraunJsonV1::parse(&summary.to_string(), Settings::default())
            .expect("Failed to parse");
        let metrics = variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "perf"}"#,
        );
        assert!(!metrics.inner.contains_key(&"faults".parse().unwrap()));
        assert!(metrics.inner.contains_key(&"cache-misses".parse().unwrap()));
        assert_eq!(results.skipped.measures, 1);
    }

    /// Kills a leaf that drops an event in a unit it does not know, names it without
    /// that unit, or creates it in any unit but the one the summary states.
    #[test]
    fn adapter_rust_gungraun_json_v7_perf_unknown_unit() {
        let mut summary = perf_summary();
        summary["profiles"][1]["data"]["parts"][0]["metrics"]["faults"]["values"]["new"]["unit"] =
            "pages".into();

        let results = AdapterRustGungraunJsonV1::parse(&summary.to_string(), Settings::default())
            .expect("Failed to parse");
        let metrics = variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "perf"}"#,
        );
        assert!(!metrics.inner.contains_key(&"faults".parse().unwrap()));
        assert_eq!(
            results.units[&"faults [pages]".parse().unwrap()].as_ref(),
            "pages"
        );
    }

    /// Kills a leaf that reads a total of the previous run in place of this run's, at
    /// schema 7 or schema 6.
    #[test]
    fn adapter_rust_gungraun_json_reads_this_run() {
        let mut v7 = perf_summary();
        v7["profiles"][0]["data"]["total"]["metrics"]["Ir"]["values"]["old"] = 1.into();
        let input =
            std::fs::read_to_string("./tool_output/rust/gungraun/json_multiple_benchmarks.txt")
                .expect("Failed to read a fixture");
        let first = input
            .lines()
            .find(|line| line.contains(r#""id":"first_id""#))
            .expect("a summary");
        let mut v6 = serde_json::from_str::<serde_json::Value>(first).expect("a summary");
        v6["profiles"][0]["summaries"]["total"]["summary"]["Callgrind"]["D1mr"]["metrics"] =
            serde_json::json!({ "Both": [{ "Int": 6 }, { "Int": 1 }] });

        let results =
            AdapterRustGungraunJsonV1::parse(&format!("{v7}\n{v6}\n"), Settings::default())
                .expect("Failed to parse");
        let instructions = variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "callgrind"}"#,
        );
        assert_eq!(
            named(instructions, gungraun::Instructions::SLUG_STR, "value"),
            Some(1802.0.into())
        );
        let misses = variant(
            &results,
            "play_game::bench_play_game_group::bench_play_game_100",
            r#"{"@id": "first_id", "@tool": "callgrind"}"#,
        );
        assert_eq!(
            named(misses, gungraun::D1mr::SLUG_STR, "value"),
            Some(6.0.into())
        );
    }

    /// Kills a leaf that skips a module path that is not a valid name without counting it.
    #[test]
    fn adapter_rust_gungraun_json_v7_invalid_module_path() {
        let mut invalid = perf_summary();
        invalid["module_path"] = "".into();
        let input = format!("{}\n{invalid}\n", perf_summary());
        let results =
            AdapterRustGungraunJsonV1::parse(&input, Settings::default()).expect("Failed to parse");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a leaf that keeps an event whose unit is not a valid unit name rather than
    /// skipping it and counting it.
    #[test]
    fn adapter_rust_gungraun_json_v7_perf_invalid_unit() {
        let mut summary = perf_summary();
        summary["profiles"][1]["data"]["parts"][0]["metrics"]["faults"]["values"]["new"]["unit"] =
            "".into();

        let results = AdapterRustGungraunJsonV1::parse(&summary.to_string(), Settings::default())
            .expect("Failed to parse");
        let metrics = variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "short", "@tool": "perf"}"#,
        );
        assert!(!metrics.inner.contains_key(&"faults []".parse().unwrap()));
        assert!(metrics.inner.contains_key(&"cache-misses".parse().unwrap()));
        assert_eq!(results.skipped.measures, 1);
    }

    /// Kills a leaf that keeps the time in system calls, whose unit the summary
    /// does not state, or skips it without counting it.
    #[test]
    fn adapter_rust_gungraun_json_v7_systime() {
        let results = convert_v1(
            Adapter::RustGungraunJson,
            "rust/gungraun/json_v7_systime.txt",
        );
        assert_variant(
            &results,
            "gung::fib_group::bench_fib",
            r#"{"@id": "long", "@tool": "callgrind"}"#,
            &expected(SYSTIME_LONG),
        );
        assert_eq!(results.skipped.measures, 5);
    }

    /// Kills a leaf that refuses schema 6, names it unlike schema 7, or leaves out
    /// its tool.
    #[test]
    fn adapter_rust_gungraun_json_v6() {
        let results = convert_v1(
            Adapter::RustGungraunJson,
            "rust/gungraun/json_multiple_benchmarks.txt",
        );
        assert_eq!(results.resolved_adapter, Some(Adapter::RustGungraunJson));
        let benchmark = "play_game::bench_play_game_group::bench_play_game_100";
        assert_variant(
            &results,
            benchmark,
            r#"{"@id": "first_id", "@tool": "callgrind"}"#,
            &[
                (gungraun::D1MissRate::SLUG_STR, vec![("value", 0.1)]),
                (gungraun::D1mr::SLUG_STR, vec![("value", 6.0)]),
            ],
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@id": "first_id", "@tool": "dhat"}"#,
            &[(gungraun::AtTEndBlocks::SLUG_STR, vec![("value", 1.0)])],
        );
        assert_variant(
            &results,
            benchmark,
            r#"{"@id": "second_id", "@tool": "memcheck"}"#,
            &[
                (gungraun::MemcheckContexts::SLUG_STR, vec![("value", 0.0)]),
                (gungraun::MemcheckErrors::SLUG_STR, vec![("value", 1.0)]),
                (
                    gungraun::MemcheckSuppressedContexts::SLUG_STR,
                    vec![("value", 2.0)],
                ),
                (
                    gungraun::MemcheckSuppressedErrors::SLUG_STR,
                    vec![("value", 3.0)],
                ),
            ],
        );
        assert_eq!(variants(&results), 3);
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_gungraun_json_v1_ignores_average() {
        for suffix in [
            "rust/gungraun/json_v7_tools.txt",
            "rust/gungraun/json_v7_perf.txt",
            "rust/gungraun/json_multiple_benchmarks.txt",
        ] {
            assert_ignores_average(Adapter::RustGungraunJson, suffix);
        }
    }

    /// The first summary of the perf fixture: `bench_fib` with `id` short, run by
    /// callgrind and then perf.
    fn perf_summary() -> serde_json::Value {
        let input = std::fs::read_to_string("./tool_output/rust/gungraun/json_v7_perf.txt")
            .expect("Failed to read a fixture");
        serde_json::from_str(input.lines().next().expect("a summary"))
            .expect("Failed to parse a summary")
    }

    /// Every perf event the fixtures name at run time: a time in nanoseconds, and a
    /// count in its own event.
    fn perf_units() -> Vec<(&'static str, &'static str)> {
        vec![
            ("branch-misses", "branch-misses"),
            ("cache-misses", "cache-misses"),
            ("context-switches", "context-switches"),
            ("cpu-clock", Latency::UNITS_STR),
            ("cycles:u", "cycles:u"),
            ("faults", "faults"),
            ("instructions:u", "instructions:u"),
            ("task-clock", Latency::UNITS_STR),
        ]
    }

    const CALLGRIND_SHORT: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 0.129366106080207),
        ("d1mr", "value", 1.0),
        ("d1mw", "value", 0.0),
        ("dlmr", "value", 0.0),
        ("dlmw", "value", 0.0),
        ("dr", "value", 415.0),
        ("dw", "value", 358.0),
        ("estimated-cycles", "value", 2647.0),
        ("i1-miss-rate", "value", 0.11098779134295228),
        ("i1mr", "value", 2.0),
        ("ilmr", "value", 2.0),
        ("instructions", "value", 1802.0),
        ("l1-hit-rate", "value", 99.88349514563106),
        ("l1-hits", "value", 2572.0),
        ("ll-hit-rate", "value", 0.03883495145631068),
        ("ll-hits", "value", 1.0),
        ("ll-miss-rate", "value", 0.07766990291262137),
        ("lld-miss-rate", "value", 0.0),
        ("lli-miss-rate", "value", 0.11098779134295228),
        ("profile-time", "value", 200554783.0),
        ("ram-hit-rate", "value", 0.07766990291262137),
        ("ram-hits", "value", 2.0),
        ("total-read-write", "value", 2575.0),
    ];
    const CALLGRIND_NO_ID: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 0.04965243296921549),
        ("d1mr", "value", 1.0),
        ("d1mw", "value", 0.0),
        ("dlmr", "value", 0.0),
        ("dlmw", "value", 0.0),
        ("dr", "value", 1080.0),
        ("dw", "value", 934.0),
        ("estimated-cycles", "value", 6839.0),
        ("i1-miss-rate", "value", 0.06357279084551812),
        ("i1mr", "value", 3.0),
        ("ilmr", "value", 3.0),
        ("instructions", "value", 4719.0),
        ("l1-hit-rate", "value", 99.9405911183722),
        ("l1-hits", "value", 6729.0),
        ("ll-hit-rate", "value", 0.014852220406950839),
        ("ll-hits", "value", 1.0),
        ("ll-miss-rate", "value", 0.04455666122085252),
        ("lld-miss-rate", "value", 0.0),
        ("lli-miss-rate", "value", 0.06357279084551812),
        ("profile-time", "value", 200735216.0),
        ("ram-hit-rate", "value", 0.04455666122085252),
        ("ram-hits", "value", 3.0),
        ("total-read-write", "value", 6733.0),
    ];
    const TOOLS_CALLGRIND: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 3.75),
        ("d1mr", "value", 1.0),
        ("d1mw", "value", 2.0),
        ("dlmr", "value", 0.0),
        ("dlmw", "value", 2.0),
        ("dr", "value", 49.0),
        ("dw", "value", 31.0),
        ("estimated-cycles", "value", 465.0),
        ("i1-miss-rate", "value", 0.8163265306122449),
        ("i1mr", "value", 2.0),
        ("ilmr", "value", 2.0),
        ("instructions", "value", 245.0),
        ("l1-hit-rate", "value", 98.46153846153847),
        ("l1-hits", "value", 320.0),
        ("ll-hit-rate", "value", 0.3076923076923077),
        ("ll-hits", "value", 1.0),
        ("ll-miss-rate", "value", 1.2307692307692308),
        ("lld-miss-rate", "value", 2.5),
        ("lli-miss-rate", "value", 0.8163265306122449),
        ("profile-time", "value", 200713957.0),
        ("ram-hit-rate", "value", 1.2307692307692308),
        ("ram-hits", "value", 4.0),
        ("total-read-write", "value", 325.0),
    ];
    const TOOLS_CACHEGRIND: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 2.6171560564168974),
        ("d1mr", "value", 2949.0),
        ("d1mw", "value", 790.0),
        ("dlmr", "value", 2120.0),
        ("dlmw", "value", 721.0),
        ("dr", "value", 101401.0),
        ("dw", "value", 41464.0),
        ("estimated-cycles", "value", 698181.0),
        ("i1-miss-rate", "value", 0.45144274735879464),
        ("i1mr", "value", 1787.0),
        ("ilmr", "value", 1738.0),
        ("instructions", "value", 395842.0),
        ("l1-hit-rate", "value", 98.97421047062689),
        ("l1-hits", "value", 533181.0),
        ("ll-hit-rate", "value", 0.17579129285492856),
        ("ll-hits", "value", 947.0),
        ("ll-miss-rate", "value", 0.8499982365181815),
        ("lld-miss-rate", "value", 1.9885906275154865),
        ("lli-miss-rate", "value", 0.4390640710182346),
        ("profile-time", "value", 200816017.0),
        ("ram-hit-rate", "value", 0.8499982365181815),
        ("ram-hits", "value", 4579.0),
        ("total-read-write", "value", 538707.0),
    ];
    const TOOLS_DHAT: &[(&str, &str, f64)] = &[
        ("at-t-end-blocks", "value", 0.0),
        ("at-t-end-bytes", "value", 0.0),
        ("at-t-gmax-blocks", "value", 0.0),
        ("at-t-gmax-bytes", "value", 0.0),
        ("maximum-blocks", "value", 0.0),
        ("maximum-bytes", "value", 0.0),
        ("profile-time", "value", 250718071.0),
        ("reads-bytes", "value", 0.0),
        ("total-blocks", "value", 1.0),
        ("total-bytes", "value", 64.0),
        ("total-lifetimes", "value", 102.0),
        ("writes-bytes", "value", 64.0),
    ];
    const TOOLS_MEMCHECK: &[(&str, &str, f64)] = &[
        ("memcheck-contexts", "value", 0.0),
        ("memcheck-errors", "value", 0.0),
        ("memcheck-suppressed-contexts", "value", 0.0),
        ("memcheck-suppressed-errors", "value", 0.0),
        ("profile-time", "value", 300816392.0),
    ];
    const TOOLS_HELGRIND: &[(&str, &str, f64)] = &[
        ("helgrind-contexts", "value", 0.0),
        ("helgrind-errors", "value", 0.0),
        ("helgrind-suppressed-contexts", "value", 0.0),
        ("helgrind-suppressed-errors", "value", 0.0),
        ("profile-time", "value", 250766791.0),
    ];
    const TOOLS_DRD: &[(&str, &str, f64)] = &[
        ("drd-contexts", "value", 0.0),
        ("drd-errors", "value", 0.0),
        ("drd-suppressed-contexts", "value", 0.0),
        ("drd-suppressed-errors", "value", 0.0),
        ("profile-time", "value", 250804719.0),
    ];
    const PERF_SHORT_CALLGRIND: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 0.129366106080207),
        ("d1mr", "value", 1.0),
        ("d1mw", "value", 0.0),
        ("dlmr", "value", 0.0),
        ("dlmw", "value", 0.0),
        ("dr", "value", 415.0),
        ("dw", "value", 358.0),
        ("estimated-cycles", "value", 2647.0),
        ("i1-miss-rate", "value", 0.11098779134295228),
        ("i1mr", "value", 2.0),
        ("ilmr", "value", 2.0),
        ("instructions", "value", 1802.0),
        ("l1-hit-rate", "value", 99.88349514563106),
        ("l1-hits", "value", 2572.0),
        ("ll-hit-rate", "value", 0.03883495145631068),
        ("ll-hits", "value", 1.0),
        ("ll-miss-rate", "value", 0.07766990291262137),
        ("lld-miss-rate", "value", 0.0),
        ("lli-miss-rate", "value", 0.11098779134295228),
        ("profile-time", "value", 200817198.0),
        ("ram-hit-rate", "value", 0.07766990291262137),
        ("ram-hits", "value", 2.0),
        ("total-read-write", "value", 2575.0),
    ];
    const PERF_SHORT_PERF: &[(&str, &str, f64)] = &[
        ("branch-misses", "value", 1106.0),
        ("cache-misses", "value", 5520.0),
        ("context-switches", "value", 1.0),
        ("cpu-clock", "value", 8410704.0),
        ("cycles:u", "value", 24392.0),
        ("faults", "value", 5.0),
        ("instructions:u", "value", 3707.0),
        ("profile-time", "value", 50916241.0),
        ("task-clock", "value", 8403995.0),
    ];
    const SYSTIME_LONG: &[(&str, &str, f64)] = &[
        ("d1-miss-rate", "value", 0.0010600069960461738),
        ("d1mr", "value", 1.0),
        ("d1mw", "value", 0.0),
        ("dlmr", "value", 0.0),
        ("dlmw", "value", 0.0),
        ("dr", "value", 50553.0),
        ("dw", "value", 43786.0),
        ("estimated-cycles", "value", 315882.0),
        ("i1-miss-rate", "value", 0.0004514570777183359),
        ("i1mr", "value", 1.0),
        ("ilmr", "value", 1.0),
        ("instructions", "value", 221505.0),
        ("l1-hit-rate", "value", 99.99936677600334),
        ("l1-hits", "value", 315842.0),
        ("ll-hit-rate", "value", 0.00031661199832828864),
        ("ll-hits", "value", 1.0),
        ("ll-miss-rate", "value", 0.00031661199832828864),
        ("lld-miss-rate", "value", 0.0),
        ("lli-miss-rate", "value", 0.0004514570777183359),
        ("profile-time", "value", 200749690.0),
        ("ram-hit-rate", "value", 0.00031661199832828864),
        ("ram-hits", "value", 1.0),
        ("sys-count", "value", 0.0),
        ("sys-cpu-time", "value", 0.0),
        ("total-read-write", "value", 315844.0),
    ];
}
