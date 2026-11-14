use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
        },
        report::Adapter,
    },
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::Finite,
            js::{rate::insert_rate, tinybench::Statistics},
            leaf::LeafResults,
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// Vitest at version 1: the JSON reporter of Vitest 5, then the `--outputJson` report of Vitest 1
/// to 4.
pub struct AdapterJsVitestV1;

impl Adaptable for AdapterJsVitestV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsVitestReporter::parse(input, settings)
            .or_else(|| AdapterJsVitestOutputJson::parse(input, settings))
    }
}

/// The report of `vitest bench --reporter=json` since Vitest 5, which nests each benchmark's
/// tasks in the test that ran them.
pub struct AdapterJsVitestReporter;

impl Adaptable for AdapterJsVitestReporter {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Reporter { test_results } = serde_json::from_str(input).ok()?;
        let root = common_directory(test_results.iter().map(|file| file.name.as_str()));
        let mut results = LeafResults::default();
        for file in &test_results {
            let path = file.name.get(root..).unwrap_or(&file.name);
            for test in &file.assertion_results {
                let chain = test
                    .ancestor_titles
                    .iter()
                    .chain([&test.title])
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                let full_test_name = chain.join(" > ");
                for benchmark in &test.benchmarks {
                    for task in &benchmark.tasks {
                        if task.from_store {
                            continue;
                        }
                        let mut name = vec![path];
                        name.extend(&chain);
                        if benchmark.name != full_test_name {
                            name.push(&benchmark.name);
                        }
                        name.push(&task.name);
                        let Ok(benchmark_name) = name.join(" > ").parse::<BenchmarkName>() else {
                            results.skip_benchmark();
                            continue;
                        };
                        let Some(mut variant) =
                            results.new_variant(benchmark_name, ParameterSet::default())
                        else {
                            continue;
                        };
                        let factor = Units::Milli.as_nanos();
                        task.latency
                            .insert(&mut variant, &Latency::name_id(), factor);
                        task.throughput
                            .insert(&mut variant, &Throughput::name_id(), 1.0);
                    }
                }
            }
        }
        results.finish_with_skips(Adapter::JsVitestV5)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reporter {
    test_results: Vec<File>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    name: String,
    assertion_results: Vec<Test>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Test {
    ancestor_titles: Vec<String>,
    title: String,
    #[serde(default)]
    benchmarks: Vec<Benchmark>,
}

#[derive(Deserialize)]
struct Benchmark {
    name: String,
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    name: String,
    latency: Statistics,
    throughput: Statistics,
    #[serde(default)]
    from_store: bool,
}

/// The length of the longest directory, with its trailing `/`, that holds every path.
fn common_directory<'a>(mut paths: impl Iterator<Item = &'a str>) -> usize {
    let Some(first) = paths.next() else {
        return 0;
    };
    let mut root = first.rfind('/').map_or(0, |slash| slash + 1);
    for path in paths {
        let shared = first
            .bytes()
            .zip(path.bytes())
            .take(root)
            .take_while(|(a, b)| a == b)
            .count();
        root = first
            .as_bytes()
            .get(..shared)
            .and_then(|prefix| prefix.iter().rposition(|byte| *byte == b'/'))
            .map_or(0, |slash| slash + 1);
    }
    root
}

/// The report of `vitest bench --outputJson` from Vitest 1 to 4, where each benchmark holds
/// tinybench's latency statistics and its rate of operations.
pub struct AdapterJsVitestOutputJson;

impl Adaptable for AdapterJsVitestOutputJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let OutputJson { files } = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for group in files.into_iter().flat_map(|file| file.groups) {
            for benchmark in group.benchmarks {
                let name = if group.full_name.is_empty() {
                    benchmark.name
                } else {
                    format!("{} > {}", group.full_name, benchmark.name)
                };
                let (Ok(benchmark_name), Some(_)) =
                    (name.parse::<BenchmarkName>(), benchmark.latency.mean())
                else {
                    results.skip_benchmark();
                    continue;
                };
                let Some(mut variant) =
                    results.new_variant(benchmark_name, ParameterSet::default())
                else {
                    continue;
                };
                benchmark.latency.insert(
                    &mut variant,
                    &Latency::name_id(),
                    Units::Milli.as_nanos(),
                );
                insert_rate(&mut variant, benchmark.hz, benchmark.rme);
            }
        }
        results.finish_with_skips(Adapter::JsVitestV1)
    }
}

#[derive(Deserialize)]
struct OutputJson {
    files: Vec<OutputFile>,
}

#[derive(Deserialize)]
struct OutputFile {
    groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
    #[serde(rename = "fullName")]
    full_name: String,
    benchmarks: Vec<OutputBenchmark>,
}

/// A benchmark with tinybench 2's statistics of its latency in milliseconds, its rate `hz`, and
/// `rme`, the margin of the mean latency as a percentage of it.
#[derive(Deserialize)]
struct OutputBenchmark {
    name: String,
    #[serde(default)]
    hz: Finite,
    #[serde(default)]
    rme: Finite,
    #[serde(flatten)]
    latency: Statistics,
}

#[cfg(test)]
mod test_js_vitest_v1 {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{assert_ignores_average, assert_variant, convert_v1},
        results::adapter_results::AdapterResults,
    };

    const REPORTER: [&str; 7] = [
        "js/vitest/v5_basic.json",
        "js/vitest/v5_errored.json",
        "js/vitest/v5_prefix.json",
        "js/vitest/v5_projects.json",
        "js/vitest/v5_samples.json",
        "js/vitest/v5_sibling.json",
        "js/vitest/v5_store.json",
    ];

    const OUTPUT_JSON: [&str; 7] = [
        "js/vitest/four.json",
        "js/vitest/two.json",
        "js/vitest/v1.json",
        "js/vitest/v2.json",
        "js/vitest/v3.json",
        "js/vitest/v4.json",
        "js/vitest/v4_errored.json",
    ];

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results.inner.keys().map(ToString::to_string).collect()
    }

    /// Kills a name with the absolute path, a path not cut at the longest shared directory (`a/`
    /// and `ab/` share `bench/`, not `bench/a`), a shared directory that leaves out a file that
    /// failed to load, a name without the describe chain, a benchmark name the test name already
    /// holds, and a custom benchmark name dropped.
    #[test]
    fn adapter_js_vitest_v5_names() {
        for (suffix, expected) in [
            (
                "js/vitest/v5_basic.json",
                vec![
                    "math.bench.ts > math > fibonacci > compare > fib(10)",
                    "math.bench.ts > math > fibonacci > compare > fib(15)",
                    "math.bench.ts > math > fibonacci > compare > fib(5)",
                    "math.bench.ts > math > fibonacci > fib(10) > recursive",
                    "math.bench.ts > top level > custom group > sum",
                    "sub/sort.bench.ts > sorting > arrays > normal",
                    "sub/sort.bench.ts > sorting > arrays > reverse",
                ],
            ),
            (
                "js/vitest/v5_prefix.json",
                vec![
                    "a/one.bench.ts > one > noop",
                    "ab/two.bench.ts > two > noop",
                ],
            ),
            (
                "js/vitest/v5_errored.json",
                vec!["errored.bench.ts > parse > numbers > parseInt"],
            ),
            (
                "js/vitest/v5_sibling.json",
                vec![
                    "bench/main.bench.ts > main > sum > loop",
                    "bench/sub/deep.bench.ts > deep > join > comma",
                ],
            ),
        ] {
            let results = convert_v1(Adapter::JsVitest, suffix);
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::JsVitestV5),
                "{suffix}"
            );
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_eq!(
                names(&results),
                expected.into_iter().map(ToOwned::to_owned).collect(),
                "{suffix}"
            );
        }
    }

    /// Kills latency left in milliseconds, a statistic dropped or misnamed, an interval not
    /// centred on the mean, and throughput read from latency.
    #[test]
    fn adapter_js_vitest_v5_statistics() {
        let results = convert_v1(Adapter::JsVitestV5, "js/vitest/v5_basic.json");
        assert_variant(
            &results,
            "sub/sort.bench.ts > sorting > arrays > reverse",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 35.404_137_468_600_986),
                        ("median", 30.000_000_037_944_12),
                        ("min", 19.999_999_949_504_854),
                        ("max", 953_292.000_000_033_1),
                        ("p75", 40.000_000_012_696_546),
                        ("p99", 50.999_999_984_924_216),
                        ("p99_5", 79.999_999_911_706_25),
                        ("p99_9", 200.999_999_947_271_14),
                        ("std_dev", 647.308_845_428_417_8),
                        ("std_err", 0.385_157_645_016_863_5),
                        ("median_abs_dev", 0.999_999_997_475_242_7),
                        ("mean_abs_dev", 7.570_725_096_439_347),
                        ("mean_ci95_lower", 34.649_242_032_510_42),
                        ("mean_ci95_upper", 36.159_032_904_691_55),
                        ("mean_ci95_margin", 0.754_895_436_090_566_4),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 30_711_373.343_282_234),
                        ("median", 33_333_333.291_173_2),
                        ("min", 1_048.996_529_919_442_5),
                        ("max", 50_000_000.126_237_86),
                        ("p75", 33_333_333.417_491_91),
                        ("p99", 49_999_999.842_020_77),
                        ("p99_5", 50_000_000.126_237_86),
                        ("p99_9", 50_000_000.126_237_86),
                        ("std_dev", 4_926_604.004_612_508),
                        ("std_err", 2_931.396_982_674_228_6),
                        ("median_abs_dev", 1_075_268.811_900_943_5),
                        ("mean_abs_dev", 3_983_455.219_951_132),
                        ("mean_ci95_lower", 30_705_627.908_309_773),
                        ("mean_ci95_upper", 30_717_118.778_254_695),
                        ("mean_ci95_margin", 5_745.434_972_460_547),
                    ],
                ),
            ],
        );
    }

    /// Kills a single file named with its directory, and samples read as a statistic.
    #[test]
    fn adapter_js_vitest_v5_single_file() {
        let results = convert_v1(Adapter::JsVitestV5, "js/vitest/v5_samples.json");
        assert_eq!(results.inner.len(), 1);
        assert_variant(
            &results,
            "samples.bench.ts > strings > concat",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 461.312_500_000_588),
                        ("median", 230.000_000_001_950_87),
                        ("min", 180.000_000_000_291_04),
                        ("max", 1_783.000_000_003_198_7),
                        ("p75", 437.750_000_003_234_16),
                        ("p99", 1_706.350_000_002_743),
                        ("p99_5", 1_744.675_000_002_971_3),
                        ("p99_9", 1_775.335_000_003_152_8),
                        ("std_dev", 462.013_812_022_656_74),
                        ("std_err", 115.503_453_005_664_18),
                        ("median_abs_dev", 40.000_000_005_591_12),
                        ("mean_abs_dev", 325.218_749_999_578_45),
                        ("mean_ci95_lower", 215.122_717_581_084_23),
                        ("mean_ci95_upper", 707.502_282_420_091_7),
                        ("mean_ci95_margin", 246.189_782_419_503_76),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 3_553_189.940_112_898_6),
                        ("median", 4_356_060.606_028_755),
                        ("min", 560_852.495_792_600_1),
                        ("max", 5_555_555.555_546_572),
                        ("p75", 4_981_343.283_464_211),
                        ("p99", 5_472_222.222_242_979),
                        ("p99_5", 5_513_888.888_894_777),
                        ("p99_9", 5_547_222.222_216_213),
                        ("std_dev", 1_717_550.727_970_797),
                        ("std_err", 429_387.681_992_699_3),
                        ("median_abs_dev", 838_870.431_950_933_8),
                        ("mean_abs_dev", 1_500_140.103_058_126_5),
                        ("mean_ci95_lower", 2_637_971.760_460_609_6),
                        ("mean_ci95_upper", 4_468_408.119_765_188),
                        ("mean_ci95_margin", 915_218.179_652_289_3),
                    ],
                ),
            ],
        );
    }

    /// Kills a task served from a store kept as a result of this run, or counted as skipped.
    #[test]
    fn adapter_js_vitest_v5_from_store() {
        let results = convert_v1(Adapter::JsVitestV5, "js/vitest/v5_store.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            names(&results),
            [
                "store.bench.ts > against baseline > join now",
                "store.bench.ts > baseline > join",
            ]
            .map(ToOwned::to_owned)
            .into()
        );
    }

    /// Kills a file a second project runs that overwrites the first project's results, or one
    /// whose repeats go uncounted: the report does not name a file's project.
    #[test]
    fn adapter_js_vitest_v5_projects() {
        let results = convert_v1(Adapter::JsVitestV5, "js/vitest/v5_projects.json");
        assert_eq!(results.skipped.variants, 2);
        assert_eq!(results.inner.len(), 2);
        for (name, mean) in [
            ("fill", 69.389_958_303_575_2),
            ("from", 353.502_941_134_885_7),
        ] {
            let metrics = results
                .get(&format!("array.bench.ts > arrays > {name}"))
                .expect("no benchmark");
            let latency = &metrics.inner[&Latency::name_id()];
            assert_eq!(latency.inner[&"mean".parse().expect("metric")], mean);
        }
    }

    /// Kills latency left in milliseconds, a statistic dropped or misnamed, and a rate interval
    /// not taken from `rme`.
    #[test]
    fn adapter_js_vitest_v1_statistics() {
        let results = convert_v1(Adapter::JsVitest, "js/vitest/v4.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsVitestV1));
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "bench/sub/sort.bench.ts > sorting > normal",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 90.635_660_221_573_96),
                        ("median", 79.999_999_968_549_67),
                        ("min", 68.999_999_996_322),
                        ("max", 1_771_480.999_999_994_4),
                        ("p75", 80.000_000_025_393_09),
                        ("p99", 240.000_000_019_335_86),
                        ("p99_5", 280.999_999_972_664_2),
                        ("p99_9", 421.000_000_017_102_14),
                        ("std_dev", 1_738.309_480_648_729_7),
                        ("std_err", 1.654_918_268_897_833_9),
                        ("mean_ci95_lower", 87.392_020_414_534_2),
                        ("mean_ci95_upper", 93.879_300_028_613_71),
                        ("mean_ci95_margin", 3.243_639_807_039_754_5),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 11_033_184.924_734_188),
                        ("mean_ci95_lower", 10_638_332.857_316_025),
                        ("mean_ci95_upper", 11_428_036.992_152_352),
                    ],
                ),
            ],
        );
    }

    /// Kills a leaf that needs the median Vitest 1 does not write.
    #[test]
    fn adapter_js_vitest_v1_without_median() {
        let results = convert_v1(Adapter::JsVitestV1, "js/vitest/v1.json");
        assert_eq!(results.inner.len(), 5);
        assert_variant(
            &results,
            "bench/math.bench.ts > top level",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 41.347_819_399_317_89),
                        ("min", 28.999_999_813_095_2),
                        ("max", 17_706.999_999_973_05),
                        ("p75", 40.000_000_126_383_384),
                        ("p99", 50.999_999_984_924_216),
                        ("p99_5", 59.999_999_848_514_56),
                        ("p99_9", 60.999_999_959_676_64),
                        ("std_dev", 43.902_033_472_093_23),
                        ("std_err", 0.028_229_999_625_567_23),
                        ("mean_ci95_lower", 41.292_488_600_051_776),
                        ("mean_ci95_upper", 41.403_150_198_584),
                        ("mean_ci95_margin", 0.055_330_799_266_111_765),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 24_185_072.260_823_913),
                        ("mean_ci95_lower", 24_152_708.295_856_945),
                        ("mean_ci95_upper", 24_217_436.225_790_88),
                    ],
                ),
            ],
        );
    }

    /// Kills a name without its group's full name.
    #[test]
    fn adapter_js_vitest_v1_names() {
        for suffix in [
            "js/vitest/v1.json",
            "js/vitest/v2.json",
            "js/vitest/v3.json",
            "js/vitest/v4.json",
        ] {
            assert_eq!(
                names(&convert_v1(Adapter::JsVitestV1, suffix)),
                [
                    "bench/math.bench.ts > math > fibonacci > fib(10)",
                    "bench/math.bench.ts > math > fibonacci > fib(15)",
                    "bench/math.bench.ts > top level",
                    "bench/sub/sort.bench.ts > sorting > normal",
                    "bench/sub/sort.bench.ts > sorting > reverse",
                ]
                .map(ToOwned::to_owned)
                .into(),
                "{suffix}"
            );
        }
    }

    /// Kills a benchmark that failed, with no statistics, refusing the whole report or going
    /// uncounted.
    #[test]
    fn adapter_js_vitest_v1_errored() {
        let results = convert_v1(Adapter::JsVitestV1, "js/vitest/v4_errored.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(
            names(&results),
            ["errbench/err.bench.ts > parse > parseInt".to_owned()].into()
        );
    }

    /// Kills a Vitest leaf that claims the other format.
    #[test]
    fn adapter_js_vitest_v1_leaves_refuse_each_other() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaf, others) in [
            (Adapter::JsVitestV5, OUTPUT_JSON.as_slice()),
            (Adapter::JsVitestV1, REPORTER.as_slice()),
        ] {
            for suffix in others {
                let input = std::fs::read_to_string(format!("./tool_output/{suffix}"))
                    .expect("Failed to read a fixture");
                assert_eq!(
                    leaf.convert(&input, settings),
                    None,
                    "{leaf} claimed {suffix}"
                );
            }
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_vitest_v1_ignores_average() {
        for suffix in REPORTER {
            assert_ignores_average(Adapter::JsVitestV5, suffix);
        }
        for suffix in OUTPUT_JSON {
            assert_ignores_average(Adapter::JsVitestV1, suffix);
        }
    }
}
