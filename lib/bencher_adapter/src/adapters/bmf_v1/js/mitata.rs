use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BranchMisses, CacheMisses, CacheReferences, Cycles, Latency},
            iai::Instructions,
            mitata::{
                Branches, GcTime, Heap, L1LoadMisses, L1StoreMisses, LoadsAndStores, StallCycles,
            },
        },
        report::Adapter,
    },
};
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant},
    },
    results::adapter_results::AdapterResults,
};

/// The report of mitata's `run({format: 'json'})`, with or without its samples.
///
/// A benchmark is named by its group's name, when it has one, and its alias, and each run's
/// `args` are its parameters, an argument that is not a scalar as its compact JSON text.
pub struct AdapterJsMitata;

impl Adaptable for AdapterJsMitata {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Report {
            layout, benchmarks, ..
        } = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for benchmark in &benchmarks {
            let group = benchmark
                .group
                .and_then(|group| layout.get(group))
                .and_then(|layout| layout.name.as_ref()?.as_str())
                .filter(|group| !group.is_empty());
            let name = match group {
                Some(group) => format!("{group} > {}", benchmark.alias),
                None => benchmark.alias.clone(),
            };
            for run in &benchmark.runs {
                let (Some(stats), Ok(benchmark_name)) = (&run.stats, name.parse::<BenchmarkName>())
                else {
                    results.skip_benchmark();
                    continue;
                };
                let args = run
                    .args
                    .iter()
                    .map(|(key, value)| (key.clone(), scalar(value)))
                    .collect();
                let Ok(parameters) =
                    serde_json::from_value::<ParameterSet>(serde_json::Value::Object(args))
                else {
                    results.skip_variant();
                    continue;
                };
                if let Some(mut variant) = results.new_variant(benchmark_name, parameters) {
                    stats.insert(&mut variant);
                }
            }
        }
        results.finish_with_skips(Adapter::JsMitata)
    }
}

/// The value, or its compact JSON text when it is not a scalar.
fn scalar(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => value.clone(),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            value.to_string().into()
        },
    }
}

#[derive(Deserialize)]
struct Report {
    layout: Vec<Layout>,
    benchmarks: Vec<Benchmark>,
    #[serde(rename = "context")]
    _context: IgnoredAny,
}

/// A group, whose `name` is not a string after a second `run()` in one process.
#[derive(Deserialize)]
struct Layout {
    name: Option<serde_json::Value>,
}

/// A benchmark, with no `group` after a second `run()` in one process.
#[derive(Deserialize)]
struct Benchmark {
    alias: String,
    group: Option<usize>,
    runs: Vec<Run>,
}

/// One run of a benchmark, with `stats`, or with the `error` it threw instead.
#[derive(Deserialize)]
struct Run {
    #[serde(default)]
    args: serde_json::Map<String, serde_json::Value>,
    stats: Option<Stats>,
}

/// The statistics of a run's samples, each the time of one iteration in nanoseconds, with the
/// heap's growth per iteration in bytes, the time of a garbage collection between samples, and
/// the hardware counters per iteration, when mitata measures them.
#[derive(Deserialize)]
struct Stats {
    avg: Finite,
    p50: Finite,
    min: Finite,
    max: Finite,
    p25: Finite,
    p75: Finite,
    p99: Finite,
    p999: Finite,
    heap: Option<Summary>,
    gc: Option<Summary>,
    counters: Option<Counters>,
}

impl Stats {
    fn insert(&self, variant: &mut Variant<'_>) {
        let Self {
            avg,
            p50,
            min,
            max,
            p25,
            p75,
            p99,
            p999,
            heap,
            gc,
            counters,
        } = self;
        let latency = Latency::name_id();
        for (metric, value) in [
            ("mean", avg),
            ("median", p50),
            ("min", min),
            ("max", max),
            ("p25", p25),
            ("p75", p75),
            ("p99", p99),
            ("p99_9", p999),
        ] {
            value.insert(variant, &latency, metric, 1.0);
        }
        if let Some(heap) = heap {
            heap.insert(variant, &Heap::name_id());
        }
        if let Some(gc) = gc {
            gc.insert(variant, &GcTime::name_id());
        }
        if let Some(counters) = counters {
            counters.insert(variant);
        }
    }
}

/// The counters of @mitata/counters: `cache` and `_bmispred` on Linux, `l1` and the cycles'
/// `stalls` and the instructions' `loads_and_stores` on macOS, and the rest on both, where
/// `branches` holds the branch misses on macOS.
#[derive(Deserialize)]
struct Counters {
    cycles: Option<CyclesCounter>,
    instructions: Option<InstructionsCounter>,
    cache: Option<CacheCounter>,
    branches: Option<BranchesCounter>,
    #[serde(rename = "_bmispred")]
    branch_misses: Option<Summary>,
    l1: Option<L1Counter>,
}

impl Counters {
    fn insert(&self, variant: &mut Variant<'_>) {
        let Self {
            cycles,
            instructions,
            cache,
            branches,
            branch_misses,
            l1,
        } = self;
        if let Some(CyclesCounter { summary, stalls }) = cycles {
            summary.insert(variant, &Cycles::name_id());
            if let Some(stalls) = stalls {
                stalls.insert(variant, &StallCycles::name_id());
            }
        }
        if let Some(InstructionsCounter {
            summary,
            loads_and_stores,
        }) = instructions
        {
            summary.insert(variant, &Instructions::name_id());
            if let Some(loads_and_stores) = loads_and_stores {
                loads_and_stores.insert(variant, &LoadsAndStores::name_id());
            }
        }
        if let Some(CacheCounter { summary, misses }) = cache {
            summary.insert(variant, &CacheReferences::name_id());
            if let Some(misses) = misses {
                misses.insert(variant, &CacheMisses::name_id());
            }
        }
        if let Some(branches) = branches {
            branches.summary.insert(variant, &Branches::name_id());
        }
        let mispredicted = branches
            .as_ref()
            .and_then(|branches| branches.mispredicted.as_ref());
        if let Some(branch_misses) = branch_misses.as_ref().or(mispredicted) {
            branch_misses.insert(variant, &BranchMisses::name_id());
        }
        if let Some(L1Counter {
            miss_loads,
            miss_stores,
        }) = l1
        {
            if let Some(miss_loads) = miss_loads {
                miss_loads.insert(variant, &L1LoadMisses::name_id());
            }
            if let Some(miss_stores) = miss_stores {
                miss_stores.insert(variant, &L1StoreMisses::name_id());
            }
        }
    }
}

#[derive(Deserialize)]
struct CyclesCounter {
    #[serde(flatten)]
    summary: Summary,
    stalls: Option<Summary>,
}

#[derive(Deserialize)]
struct InstructionsCounter {
    #[serde(flatten)]
    summary: Summary,
    loads_and_stores: Option<Summary>,
}

#[derive(Deserialize)]
struct CacheCounter {
    #[serde(flatten)]
    summary: Summary,
    misses: Option<Summary>,
}

#[derive(Deserialize)]
struct BranchesCounter {
    #[serde(flatten)]
    summary: Summary,
    mispredicted: Option<Summary>,
}

#[derive(Deserialize)]
struct L1Counter {
    miss_loads: Option<Summary>,
    miss_stores: Option<Summary>,
}

/// The mean, minimum, and maximum of a quantity over a run's samples.
#[derive(Deserialize)]
struct Summary {
    avg: Finite,
    min: Finite,
    max: Finite,
}

impl Summary {
    fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId) {
        let Self { avg, min, max } = self;
        for (metric, value) in [("mean", avg), ("min", min), ("max", max)] {
            value.insert(variant, measure, metric, 1.0);
        }
    }
}

#[cfg(test)]
mod test_js_mitata {
    use std::collections::BTreeSet;

    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BranchMisses, CacheMisses, CacheReferences, Cycles, Latency},
            iai::Instructions,
            mitata::{
                Branches, GcTime, Heap, L1LoadMisses, L1StoreMisses, LoadsAndStores, StallCycles,
            },
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_value, assert_variant, convert_edited_v1, convert_v1,
            variants,
        },
        results::adapter_results::AdapterResults,
    };

    fn variant_names(results: &AdapterResults) -> BTreeSet<(String, String)> {
        results
            .inner
            .iter()
            .flat_map(|(name, variants)| {
                variants
                    .keys()
                    .map(move |parameters| (name.to_string(), parameters.canonical()))
            })
            .collect()
    }

    /// Kills a name from the run's filled-in name, a group's name dropped or put after the alias,
    /// an `args` key renamed or typed as a string, a run that threw kept or left uncounted, and a
    /// repeated name and `args` that overwrites the first run or goes uncounted.
    #[test]
    fn adapter_js_mitata_names() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/node.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsMitata));
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 1)
        );
        let expected = [
            ("barred", "{}"),
            ("compacted", "{}"),
            ("computed parameters", "{}"),
            ("concat $a $b", r#"{"a":1,"b":"x"}"#),
            ("concat $a $b", r#"{"a":1,"b":"y"}"#),
            ("concat $a $b", r#"{"a":2,"b":"x"}"#),
            ("concat $a $b", r#"{"a":2,"b":"y"}"#),
            ("dense", r#"{"n":1}"#),
            ("dense", r#"{"n":2}"#),
            ("dense", r#"{"n":3}"#),
            ("fill $size", r#"{"size":1}"#),
            ("fill $size", r#"{"size":1024}"#),
            ("fill $size", r#"{"size":512}"#),
            ("fill $size", r#"{"size":64}"#),
            ("fill $size", r#"{"size":8}"#),
            ("inner gc", "{}"),
            ("noop", "{}"),
            ("parsing > json parse", "{}"),
            ("parsing > url parse", "{}"),
            ("boxed", "{}"),
            ("same name", "{}"),
            ("sleep 20ms", "{}"),
            ("sort 1000", "{}"),
            ("unnamed args", r#"{"x":"three"}"#),
            ("unnamed args", r#"{"x":1}"#),
            ("unnamed args", r#"{"x":2.5}"#),
        ]
        .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
        .into();
        assert_eq!(variant_names(&results), expected);
        assert_eq!(variants(&results), 26);
        assert_value(
            &results,
            ("same name", "{}"),
            (Latency::SLUG_STR, "mean"),
            0.053_711_910_417_369_35,
        );
    }

    /// Kills a statistic dropped, misnamed, or scaled, the heap's growth read from its total or
    /// its count, and `ticks` written as a metric.
    #[test]
    fn adapter_js_mitata_statistics() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/node.json");
        assert_variant(
            &results,
            "fill $size",
            r#"{"size":64}"#,
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 84.578_128_902_347_66),
                        ("median", 73.685_546_875),
                        ("min", 62.056_152_343_75),
                        ("max", 718.683_837_890_625),
                        ("p25", 70.181_640_625),
                        ("p75", 79.060_058_593_75),
                        ("p99", 248.883_300_781_25),
                        ("p99_9", 652.235_595_703_125),
                    ],
                ),
                (
                    Heap::SLUG_STR,
                    vec![
                        ("mean", 560.086_546_930_045_3),
                        ("min", 363.945_312_5),
                        ("max", 688.130_859_375),
                    ],
                ),
            ],
        );
    }

    /// Kills the default form, with every sample and its debug text, refused, and the time of a
    /// garbage collection dropped, misnamed, or read with its total, which grows with the run.
    #[test]
    fn adapter_js_mitata_default_form_and_gc() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/default_gc.json");
        assert_eq!(variants(&results), 26);
        assert_variant(
            &results,
            "inner gc",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 705.083_343_505_859_4),
                        ("median", 704.274_414_062_5),
                        ("min", 543.251_464_843_75),
                        ("max", 931.004_150_390_625),
                        ("p25", 633.031_494_140_625),
                        ("p75", 772.399_658_203_125),
                        ("p99", 883.352_294_921_875),
                        ("p99_9", 929.060_546_875),
                    ],
                ),
                (
                    Heap::SLUG_STR,
                    vec![
                        ("mean", 8_211.783_285_884_534),
                        ("min", 7_676.240_234_375),
                        ("max", 8_762.958_984_375),
                    ],
                ),
                (
                    GcTime::SLUG_STR,
                    vec![
                        ("mean", 1_604_139.635_593_220_4),
                        ("min", 891_247.0),
                        ("max", 4_434_142.0),
                    ],
                ),
            ],
        );
    }

    /// Kills a counter dropped or put in another counter's measure, the cache misses read from the
    /// references, and the absent `branches` counted as skipped.
    #[test]
    fn adapter_js_mitata_counters() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/counters.json");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "fill $size",
            r#"{"size":4096}"#,
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 3_786.628_405_448_718),
                        ("median", 3_724.609_863_281_25),
                        ("min", 2_856.294_921_875),
                        ("max", 7_819.337_890_625),
                        ("p25", 3_208.903_808_593_75),
                        ("p75", 4_036.368_896_484_375),
                        ("p99", 5_623.241_455_078_125),
                        ("p99_9", 5_623.241_455_078_125),
                    ],
                ),
                (
                    Heap::SLUG_STR,
                    vec![
                        ("mean", 17_465.708_007_812_5),
                        ("min", 4_711.677_734_375),
                        ("max", 32_816.099_609_375),
                    ],
                ),
                (
                    Cycles::SLUG_STR,
                    vec![
                        ("mean", 16_347.886_145_303_415),
                        ("min", 12_682.868_896_484_375),
                        ("max", 23_866.776_123_046_875),
                    ],
                ),
                (
                    Instructions::SLUG_STR,
                    vec![
                        ("mean", 65_382.442_291_969_48),
                        ("min", 62_870.821_533_203_125),
                        ("max", 83_962.384_033_203_12),
                    ],
                ),
                (
                    CacheReferences::SLUG_STR,
                    vec![
                        ("mean", 1_351.802_422_102_107_6),
                        ("min", 1_127.429_931_640_625),
                        ("max", 1_887.949_462_890_625),
                    ],
                ),
                (
                    CacheMisses::SLUG_STR,
                    vec![
                        ("mean", 56.350_750_590_479_65),
                        ("min", 33.853_027_343_75),
                        ("max", 108.485_595_703_125),
                    ],
                ),
                (
                    BranchMisses::SLUG_STR,
                    vec![
                        ("mean", 7.140_500_090_843_023),
                        ("min", 2.471_923_828_125),
                        ("max", 22.332_031_25),
                    ],
                ),
            ],
        );
    }

    /// Kills the zeros an unprivileged run reads dropped or counted as skipped.
    #[test]
    fn adapter_js_mitata_counters_unprivileged() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/counters_unprivileged.json");
        assert!(results.skipped.is_empty());
        for measure in [
            Cycles::SLUG_STR,
            Instructions::SLUG_STR,
            CacheReferences::SLUG_STR,
            CacheMisses::SLUG_STR,
            BranchMisses::SLUG_STR,
        ] {
            assert_value(&results, ("sort 1000", "{}"), (measure, "max"), 0.0);
        }
    }

    /// Kills the macOS branch misses dropped or read from the count of branches, a counter with no
    /// Bencher unit dropped, counted as skipped, or put in another counter's measure, the two L1
    /// misses written to one measure, and a null counter written: the counters of
    /// @mitata/counters 0.0.8 on macOS.
    #[test]
    fn adapter_js_mitata_counters_macos() {
        let event = |avg: f64| serde_json::json!({ "min": 1.0, "max": 9.0, "avg": avg });
        let results = convert_edited_v1(Adapter::JsMitata, "js/mitata/counters.json", |json| {
            json["benchmarks"][0]["runs"][0]["stats"]["counters"] = serde_json::json!({
                "l1": { "miss_loads": event(2.0), "miss_stores": null },
                "cycles": { "min": 1.0, "max": 9.0, "avg": 3.0, "stalls": event(4.0) },
                "branches": { "min": 1.0, "max": 9.0, "avg": 5.0, "mispredicted": event(6.0) },
                "instructions": { "min": 1.0, "max": 9.0, "avg": 7.0, "loads_and_stores": event(8.0) },
            });
        });
        assert!(results.skipped.is_empty());
        for (measure, mean) in [
            (Cycles::SLUG_STR, 3.0),
            (StallCycles::SLUG_STR, 4.0),
            (Branches::SLUG_STR, 5.0),
            (BranchMisses::SLUG_STR, 6.0),
            (Instructions::SLUG_STR, 7.0),
            (LoadsAndStores::SLUG_STR, 8.0),
            (L1LoadMisses::SLUG_STR, 2.0),
        ] {
            for (metric, value) in [("mean", mean), ("min", 1.0), ("max", 9.0)] {
                assert_value(&results, ("sort 1000", "{}"), (measure, metric), value);
            }
        }
        let metrics = results.get("sort 1000").expect("no benchmark");
        assert!(!metrics.inner.contains_key(&CacheReferences::name_id()));
        assert!(!metrics.inner.contains_key(&L1StoreMisses::name_id()));
        let results = convert_edited_v1(Adapter::JsMitata, "js/mitata/counters.json", |json| {
            json["benchmarks"][0]["runs"][0]["stats"]["counters"] = serde_json::json!({
                "l1": { "miss_loads": null, "miss_stores": event(3.0) },
            });
        });
        assert_value(
            &results,
            ("sort 1000", "{}"),
            (L1StoreMisses::SLUG_STR, "mean"),
            3.0,
        );
    }

    /// Kills Linux's count of branches dropped, when the kernel measures it, and the branch
    /// misses read from it.
    #[test]
    fn adapter_js_mitata_counters_linux_branches() {
        let results = convert_edited_v1(Adapter::JsMitata, "js/mitata/counters.json", |json| {
            let counters = &mut json["benchmarks"][0]["runs"][0]["stats"]["counters"];
            counters["branches"] = serde_json::json!({
                "mispredicted": counters["_bmispred"].clone(),
                "min": 100.0,
                "max": 900.0,
                "avg": 500.0,
            });
        });
        assert!(results.skipped.is_empty());
        assert_value(
            &results,
            ("sort 1000", "{}"),
            (Branches::SLUG_STR, "mean"),
            500.0,
        );
        assert_value(
            &results,
            ("sort 1000", "{}"),
            (BranchMisses::SLUG_STR, "mean"),
            1_235.607_252_472_433_9,
        );
    }

    /// Kills an `args` value that is not a scalar dropped, skipping its run, or written as
    /// anything but its compact JSON text with its keys sorted, and an over-long text kept or
    /// left uncounted.
    #[test]
    fn adapter_js_mitata_non_scalar_arg() {
        let results = convert_edited_v1(Adapter::JsMitata, "js/mitata/node.json", |json| {
            let runs = &mut json["benchmarks"][6]["runs"];
            runs[0]["args"]["size"] = serde_json::json!([1, 2, 3]);
            runs[1]["args"]["size"] = serde_json::json!({ "b": 1, "a": [2] });
            runs[2]["args"]["size"] = serde_json::Value::Null;
            runs[3]["args"]["size"] = serde_json::json!(vec![1_000; 64]);
        });
        assert_eq!(results.skipped.variants, 2);
        let sizes = variant_names(&results)
            .into_iter()
            .filter(|(name, _)| name == "fill $size")
            .map(|(_, parameters)| parameters)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            sizes,
            [
                r#"{"size":"[1,2,3]"}"#,
                r#"{"size":"null"}"#,
                r#"{"size":"{\"a\":[2],\"b\":1}"}"#,
                r#"{"size":1024}"#,
            ]
            .map(ToOwned::to_owned)
            .into()
        );
    }

    /// Kills a report from a second `run()` in one process, whose layout names are the number 0
    /// and whose top-level benchmark has no `group`, refused, or a number written as a group's
    /// name, as for the benchmark in its `boxplot` block.
    #[test]
    fn adapter_js_mitata_second_run() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/second_run.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            variant_names(&results),
            [
                ("boxed fill", "{}"),
                ("formatting > json stringify", "{}"),
                ("sum $n", r#"{"n":64}"#),
                ("sum $n", r#"{"n":8}"#),
            ]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
    }

    /// Kills a group named the empty string, as mitata writes for `group('', ...)`, put before
    /// the alias as an empty part of the name.
    #[test]
    fn adapter_js_mitata_empty_group_name() {
        let results = convert_v1(Adapter::JsMitata, "js/mitata/empty_group.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            variant_names(&results),
            [
                ("noop", "{}"),
                ("parsing > json parse", "{}"),
                ("url parse", "{}")
            ]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_mitata_ignores_average() {
        for suffix in [
            "js/mitata/node.json",
            "js/mitata/default_gc.json",
            "js/mitata/counters.json",
        ] {
            assert_ignores_average(Adapter::JsMitata, suffix);
        }
    }
}
