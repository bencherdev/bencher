use std::{borrow::Cow, collections::BTreeMap};

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{
                BranchMisses, CacheMisses, CacheReferences, CpuTime, Cycles, Latency, PeakMemory,
                SystemTime, UserTime,
            },
            iai::Instructions,
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
            leaf::{LeafResults, Variant},
            runs::statistics,
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// hyperfine at version 1: its 2.x JSON export, then its 1.x JSON export.
///
/// A run is named for its `--command-name`, else its command, and its `--parameter-scan` and
/// `--parameter-list` values and its `--env` variables are its parameters, as strings.
pub struct AdapterShellHyperfineV1;

impl Adaptable for AdapterShellHyperfineV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterShellHyperfine2::parse(input, settings)
            .or_else(|| AdapterShellHyperfine1::parse(input, settings))
    }
}

/// The JSON export of hyperfine 2.x, whose `summary` holds the statistics of every metric.
pub struct AdapterShellHyperfine2;

impl Adaptable for AdapterShellHyperfine2 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Export { results: runs } = serde_json::from_str::<Export<Result2>>(input).ok()?;
        let mut results = LeafResults::default();
        for Result2 {
            command,
            name,
            parameters,
            environment,
            summary,
        } in runs
        {
            let name = run_name(name.as_deref(), &command, &environment);
            let mut parameters = parameters
                .into_iter()
                .map(|(key, Parameter { value })| (key, value.into()))
                .collect::<serde_json::Map<_, _>>();
            let mut repeated = false;
            for (variable, value) in environment {
                repeated |= parameters.insert(variable, value.into()).is_some();
            }
            if repeated {
                results.skip_variant();
                continue;
            }
            let Some(mut variant) = new_variant(&mut results, name, parameters) else {
                continue;
            };
            for (metric, summary) in summary {
                if let Some((measure, factor)) = measure_of(&metric) {
                    summary.insert(&mut variant, &measure, factor);
                } else {
                    variant.skip_measure();
                }
            }
        }
        results.finish_with_skips(Adapter::ShellHyperfineV2)
    }
}

/// The JSON export of hyperfine 1.x, which summarizes only the wall clock time, and whose peak
/// memory of each run is summarized here as hyperfine 2 summarizes it.
pub struct AdapterShellHyperfine1;

impl Adaptable for AdapterShellHyperfine1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Export { results: runs } = serde_json::from_str::<Export<Result1>>(input).ok()?;
        let mut results = LeafResults::default();
        for Result1 {
            command,
            latency,
            user,
            system,
            memory_usage_byte,
            parameters,
        } in runs
        {
            let parameters = parameters
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect();
            let Some(mut variant) = new_variant(&mut results, &command, parameters) else {
                continue;
            };
            let factor = Units::Sec.as_nanos();
            latency.insert(&mut variant, &Latency::name_id(), factor);
            user.insert(&mut variant, &UserTime::name_id(), "mean", factor);
            system.insert(&mut variant, &SystemTime::name_id(), "mean", factor);
            if let Some(memory) = memory_usage_byte.and_then(Summary::of) {
                memory.insert(&mut variant, &PeakMemory::name_id(), 1.0);
            }
        }
        results.finish_with_skips(Adapter::ShellHyperfineV1)
    }
}

#[derive(Deserialize)]
struct Export<R> {
    results: Vec<R>,
}

#[derive(Deserialize)]
struct Result2 {
    command: String,
    name: Option<String>,
    #[serde(default)]
    parameters: BTreeMap<String, Parameter>,
    #[serde(default)]
    environment: BTreeMap<String, String>,
    summary: BTreeMap<String, Summary>,
}

#[derive(Deserialize)]
struct Parameter {
    value: String,
}

#[derive(Deserialize)]
struct Result1 {
    command: String,
    #[serde(flatten)]
    latency: Summary,
    user: Finite,
    system: Finite,
    memory_usage_byte: Option<Vec<f64>>,
    #[serde(default)]
    parameters: BTreeMap<String, String>,
}

/// The statistics hyperfine summarizes a metric's runs with, the deviation null for one run.
#[derive(Deserialize)]
struct Summary {
    mean: Finite,
    #[serde(default)]
    stddev: Finite,
    median: Finite,
    min: Finite,
    max: Finite,
}

impl Summary {
    /// The statistics of the runs, as hyperfine 2 computes them.
    fn of(runs: Vec<f64>) -> Option<Self> {
        if let [run] = runs.as_slice() {
            let run = Finite::from(Some(*run));
            return Some(Self {
                mean: run,
                stddev: Finite::default(),
                median: run,
                min: run,
                max: run,
            });
        }
        let [mean, median, min, max, std_dev] =
            statistics(runs)?.map(|(_, value)| Finite::from(Some(value)));
        Some(Self {
            mean,
            stddev: std_dev,
            median,
            min,
            max,
        })
    }

    fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId, factor: f64) {
        let Self {
            mean,
            stddev,
            median,
            min,
            max,
        } = *self;
        for (name, value) in [
            ("mean", mean),
            ("median", median),
            ("min", min),
            ("max", max),
        ] {
            value.insert(variant, measure, name, factor);
        }
        stddev.insert_spread(mean, variant, measure, "std_dev", factor);
    }
}

/// The run's name without the `NAME=value ` of each `--env` variable that hyperfine puts before
/// an automatic name, in the order the user set them.
fn run_name<'a>(
    name: Option<&'a str>,
    command: &'a str,
    environment: &BTreeMap<String, String>,
) -> &'a str {
    let Some(name) = name else {
        return command;
    };
    let mut rest = name;
    let mut variables = environment.iter().collect::<Vec<_>>();
    while !variables.is_empty() {
        let Some((index, after)) =
            variables
                .iter()
                .enumerate()
                .find_map(|(index, (variable, value))| {
                    let after = rest
                        .strip_prefix(variable.as_str())?
                        .strip_prefix('=')?
                        .strip_prefix(quote(value).as_ref())?
                        .strip_prefix(' ')?;
                    Some((index, after))
                })
        else {
            return name;
        };
        variables.swap_remove(index);
        rest = after;
    }
    if rest == command { command } else { name }
}

/// A value quoted as hyperfine quotes it in a name with `shell-words`.
fn quote(value: &str) -> Cow<'_, str> {
    const SPECIAL: [char; 22] = [
        '\n', '\'', '|', '&', ';', '<', '>', '(', ')', '$', '`', '\\', '"', ' ', '\t', '*', '?',
        '[', '#', '~', '=', '%',
    ];
    if value.is_empty() || value.contains(SPECIAL) {
        Cow::Owned(format!("'{}'", value.replace('\'', r"'\''")))
    } else {
        Cow::Borrowed(value)
    }
}

/// The run's variant, or `None`, counted as skipped, when its name or parameters are not valid
/// or it repeats a run already read.
fn new_variant<'a>(
    results: &'a mut LeafResults,
    name: &str,
    parameters: serde_json::Map<String, serde_json::Value>,
) -> Option<Variant<'a>> {
    let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
        results.skip_benchmark();
        return None;
    };
    let Ok(parameters) =
        serde_json::from_value::<ParameterSet>(serde_json::Value::Object(parameters))
    else {
        results.skip_variant();
        return None;
    };
    results.new_variant(benchmark_name, parameters)
}

/// The measure of a hyperfine 2 metric, and the factor from hyperfine's unit to the measure's.
fn measure_of(metric: &str) -> Option<(MeasureNameId, f64)> {
    let seconds = Units::Sec.as_nanos();
    Some(match metric {
        "time_wall_clock" => (Latency::name_id(), seconds),
        "time_cpu" => (CpuTime::name_id(), seconds),
        "time_user" => (UserTime::name_id(), seconds),
        "time_system" => (SystemTime::name_id(), seconds),
        "memory_peak_resident" => (PeakMemory::name_id(), 1.0),
        "cpu_cycles" => (Cycles::name_id(), 1.0),
        "instructions" => (Instructions::name_id(), 1.0),
        "cache_references" => (CacheReferences::name_id(), 1.0),
        "cache_misses" => (CacheMisses::name_id(), 1.0),
        "branch_misses" => (BranchMisses::name_id(), 1.0),
        _ => return None,
    })
}

#[cfg(test)]
mod test_shell_hyperfine_v1 {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{
                BranchMisses, CacheMisses, CacheReferences, CpuTime, Cycles, Latency, PeakMemory,
                SystemTime, UserTime,
            },
            iai::Instructions,
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{
        Expected, assert_ignores_average, assert_value, assert_variant, convert_edited_v1,
        convert_v1, variants,
    };

    const COUNTERS: &str = "shell/hyperfine/v2_counters.json";

    /// The five statistics hyperfine summarizes a metric with, in its order: mean, standard
    /// deviation, median, minimum, and maximum.
    fn summary(measure: &'static str, statistics: [f64; 5]) -> Expected {
        let [mean, std_dev, median, min, max] = statistics;
        (
            measure,
            vec![
                ("mean", mean),
                ("std_dev", std_dev),
                ("median", median),
                ("min", min),
                ("max", max),
            ],
        )
    }

    /// Kills a hyperfine 2 metric dropped or put in the wrong measure, a time left in seconds, a
    /// statistic dropped or misnamed, the counters given measures of their own rather than the
    /// built-ins of the same quantity, and a leaf that records no leaf.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_metrics() {
        let results = convert_v1(Adapter::ShellHyperfineV2, COUNTERS);
        assert_eq!(results.resolved_adapter, Some(Adapter::ShellHyperfineV2));
        assert!(results.skipped.is_empty());
        let cpu = [
            4_970_333.333_333_333,
            6_801_022.374_123_86,
            1_119_000.0,
            968_999.999_999_999_9,
            12_823_000.0,
        ];
        assert_variant(
            &results,
            "sleep 0.001",
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [
                        6_064_037.333_333_334,
                        6_811_594.490_753_126,
                        2_201_129.0,
                        2_062_004.0,
                        13_928_979.000_000_001,
                    ],
                ),
                summary(CpuTime::SLUG_STR, cpu),
                summary(UserTime::SLUG_STR, [0.0; 5]),
                summary(SystemTime::SLUG_STR, cpu),
                summary(
                    PeakMemory::SLUG_STR,
                    [3_407_872.0, 0.0, 3_407_872.0, 3_407_872.0, 3_407_872.0],
                ),
                summary(
                    Cycles::SLUG_STR,
                    [
                        384_548.666_666_666_7,
                        24_638.890_018_288_836,
                        378_314.0,
                        363_626.0,
                        411_706.0,
                    ],
                ),
                summary(
                    Instructions::SLUG_STR,
                    [
                        328_094.0,
                        426.253_445_733_873_2,
                        327_860.0,
                        327_836.0,
                        328_586.0,
                    ],
                ),
                summary(
                    CacheReferences::SLUG_STR,
                    [
                        26_903.0,
                        439.449_655_819_639_93,
                        26_683.0,
                        26_617.0,
                        27_409.0,
                    ],
                ),
                summary(
                    CacheMisses::SLUG_STR,
                    [
                        5_528.666_666_666_667,
                        148.850_036_390_097_44,
                        5_479.0,
                        5_411.0,
                        5_696.0,
                    ],
                ),
                summary(
                    BranchMisses::SLUG_STR,
                    [
                        4_476.333_333_333_333,
                        87.185_625_726_568_79,
                        4_427.0,
                        4_425.0,
                        4_577.0,
                    ],
                ),
            ],
        );
    }

    /// Kills the `parameters` of a run dropped, so that runs sharing a `--command-name` overwrite
    /// one another (issue 472), a value typed as a number, a value read from the wrong run, and
    /// the custom name ignored for the command.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_parameters() {
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_same_name.json",
        );
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 2);
        for (ms, median) in [("1", 1_740_070.0), ("2", 2_779_657.0)] {
            assert_value(
                &results,
                ("sleep", &format!(r#"{{"ms":"{ms}"}}"#)),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_two_lists.json",
        );
        assert!(results.skipped.is_empty());
        for (command, a, b, median) in [
            ("sleep 0.001", "1", "x", 1_759_500.000_000_000_2),
            ("sleep 0.002", "2", "x", 2_647_464.0),
            ("sleep 0.001", "1", "y", 1_924_744.000_000_000_2),
            ("sleep 0.002", "2", "y", 2_723_220.0),
        ] {
            assert_value(
                &results,
                (command, &format!(r#"{{"a":"{a}","b":"{b}"}}"#)),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills an `--env` variable dropped, so that runs under each environment repeat one another,
    /// or keyed by anything but its name, the `NAME=value ` hyperfine puts before an automatic
    /// name kept, or cut from a name without quoting its value as hyperfine does or in the
    /// variables' sorted order rather than hyperfine's, a `--command-name` cut, even one that
    /// starts as an automatic name does, and a variable that
    /// repeats a `--parameter-scan` name stored over it or left uncounted.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_environment() {
        for (suffix, expected) in [
            (
                "v2_env.json",
                [
                    ("sleep 0.001", r#"{"MODE":"fast"}"#, 1_735_814.0),
                    ("sleep 0.001", r#"{"MODE":"slow"}"#, 1_738_227.000_000_000_2),
                ],
            ),
            (
                "v2_env_quoted.json",
                [
                    (
                        "sleep 0.001",
                        r#"{"ALPHA":"two words","EQ":"a=b","Q":"it's","ZED":"1"}"#,
                        1_513_563.0,
                    ),
                    ("sleep 0.002", r#"{"MODE":"a"}"#, 2_628_028.0),
                ],
            ),
            (
                "v2_env_scan.json",
                [
                    ("sleep 0.001", r#"{"n":"1","N":"1"}"#, 1_490_334.0),
                    (
                        "sleep 0.002",
                        r#"{"n":"2","N":"2"}"#,
                        2_468_124.000_000_000_5,
                    ),
                ],
            ),
        ] {
            let results = convert_v1(
                Adapter::ShellHyperfineV2,
                &format!("shell/hyperfine/{suffix}"),
            );
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_eq!(variants(&results), 2, "{suffix}");
            for (name, parameters, median) in expected {
                assert_value(
                    &results,
                    (name, parameters),
                    (Latency::SLUG_STR, "median"),
                    median,
                );
            }
        }
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_env_named.json",
        );
        assert_eq!(variants(&results), 2);
        for (name, median) in [
            ("custom", 1_488_506.000_000_000_2),
            ("MODE=a quick", 2_485_722.0),
        ] {
            assert_value(
                &results,
                (name, r#"{"MODE":"a"}"#),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_env_clash.json",
        );
        assert_eq!(results.skipped.variants, 2);
        assert!(results.inner.is_empty());
    }

    /// Kills an `--env` variable set empty skipped, counted, or stored as if unset, and the
    /// `NAME='' ` hyperfine puts before an automatic name for it kept, while a command run with
    /// no `--env` keeps no key.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_empty_environment() {
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_env_empty.json",
        );
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 2);
        for (name, parameters, median) in [
            ("sleep 0.001", r#"{"X":""}"#, 1_505_282.000_000_000_2),
            ("sleep 0.002", "{}", 2_520_959.0),
        ] {
            assert_value(
                &results,
                (name, parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills the null standard deviation of a single run stored or counted as skipped.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_one_run() {
        let results = convert_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_scan_one_run.json",
        );
        assert!(results.skipped.is_empty());
        let metrics = &results.inner[&"sleep 0.001".parse().expect("Failed to parse")]
            [&r#"{"n":"1"}"#.parse().expect("Failed to parse")];
        assert_eq!(metrics.inner.len(), 10);
        for (measure, metric) in &metrics.inner {
            let names = metric
                .inner
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            assert_eq!(names, ["max", "mean", "median", "min"], "{measure}");
        }
        assert_value(
            &results,
            ("sleep 0.001", r#"{"n":"1"}"#),
            (Latency::SLUG_STR, "mean"),
            1_912_416.000_000_000_1,
        );
    }

    /// Kills a run that repeats a name and parameters already read stored over the first or
    /// left uncounted, in either format.
    #[test]
    fn adapter_shell_hyperfine_v1_repeats() {
        let results = convert_edited_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_named.json",
            |json| {
                json["results"][1]["name"] = "fast".into();
            },
        );
        assert_eq!(results.skipped.variants, 1);
        assert_value(
            &results,
            ("fast", "{}"),
            (Latency::SLUG_STR, "median"),
            10_879_109.000_000_002,
        );
        let results = convert_edited_v1(
            Adapter::ShellHyperfineV1,
            "shell/hyperfine/v1_scan.json",
            |json| {
                json["results"][1]["command"] = "sleep 0.001".into();
                json["results"][1]["parameters"]["ms"] = "1".into();
            },
        );
        assert_eq!(results.skipped.variants, 1);
        assert_value(
            &results,
            ("sleep 0.001", r#"{"ms":"1"}"#),
            (Latency::SLUG_STR, "median"),
            1_485_811.0,
        );
    }

    /// Kills a run with more than 8 parameters, or a value past 64 characters, stored or left
    /// uncounted, in either format.
    #[test]
    fn adapter_shell_hyperfine_v1_parameter_limits() {
        let nine = (0..9)
            .map(|key| (format!("p{key}"), serde_json::json!({ "value": "1" })))
            .collect::<serde_json::Map<_, _>>();
        let results = convert_edited_v1(
            Adapter::ShellHyperfineV2,
            "shell/hyperfine/v2_same_name.json",
            |json| {
                json["results"][0]["parameters"] = nine.into();
                json["results"][1]["parameters"]["ms"]["value"] = "x".repeat(65).into();
            },
        );
        assert_eq!(results.skipped.variants, 2);
        assert!(results.inner.is_empty());
        let results = convert_edited_v1(
            Adapter::ShellHyperfineV1,
            "shell/hyperfine/v1_scan.json",
            |json| {
                json["results"][0]["parameters"]["ms"] = "x".repeat(65).into();
            },
        );
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(variants(&results), 1);
    }

    /// Kills a metric a later hyperfine adds dropped uncounted.
    #[test]
    fn adapter_shell_hyperfine_v1_v2_unknown_metric() {
        let results = convert_edited_v1(Adapter::ShellHyperfineV2, COUNTERS, |json| {
            json["results"][0]["summary"]["time_gpu"] =
                json["results"][0]["summary"]["time_cpu"].clone();
        });
        assert_eq!(results.skipped.measures, 1);
    }

    /// Kills hyperfine 1's latency statistics recomputed or dropped, its user and system times
    /// read as anything but means, the peak memory derived with a population deviation or the
    /// upper middle run as its median, and a leaf that records no leaf.
    #[test]
    fn adapter_shell_hyperfine_v1_v1_metrics() {
        let results = convert_v1(Adapter::ShellHyperfineV1, "shell/hyperfine/v1_memory.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::ShellHyperfineV1));
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            r#"python3 -c "import random; x = bytearray(random.randint(1, 64) * 1000000)""#,
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [
                        16_972_080.0,
                        3_461_028.910_115_314,
                        17_015_026.000_000_004,
                        12_870_464.000_000_002,
                        20_987_804.000_000_004,
                    ],
                ),
                (UserTime::SLUG_STR, vec![("mean", 9_482_500.0)]),
                (SystemTime::SLUG_STR, vec![("mean", 7_424_000.0)]),
                summary(
                    PeakMemory::SLUG_STR,
                    [
                        40_730_624.0,
                        18_899_950.662_396_554,
                        44_957_696.0,
                        17_039_360.0,
                        55_967_744.0,
                    ],
                ),
            ],
        );
    }

    /// Kills a single run's peak memory written as `value` rather than in hyperfine 2's shape,
    /// and its null standard deviation stored or counted.
    #[test]
    fn adapter_shell_hyperfine_v1_v1_one_run() {
        let results = convert_v1(Adapter::ShellHyperfineV1, "shell/hyperfine/v1_one_run.json");
        assert!(results.skipped.is_empty());
        let one = |value: f64| {
            vec![
                ("mean", value),
                ("median", value),
                ("min", value),
                ("max", value),
            ]
        };
        assert_variant(
            &results,
            "sleep 0.001",
            "{}",
            &[
                (Latency::SLUG_STR, one(1_472_670.000_000_000_2)),
                (UserTime::SLUG_STR, vec![("mean", 0.0)]),
                (SystemTime::SLUG_STR, vec![("mean", 384_000.0)]),
                (PeakMemory::SLUG_STR, one(3_276_800.0)),
            ],
        );
    }

    /// Kills hyperfine 1's `parameters` dropped (issue 472) or typed as numbers.
    #[test]
    fn adapter_shell_hyperfine_v1_v1_parameters() {
        let results = convert_v1(
            Adapter::ShellHyperfineV1,
            "shell/hyperfine/v1_same_name.json",
        );
        assert!(results.skipped.is_empty());
        for (ms, median) in [("1", 1_469_326.000_000_000_2), ("2", 2_452_094.5)] {
            assert_value(
                &results,
                ("sleep", &format!(r#"{{"ms":"{ms}"}}"#)),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a peak memory derived from hyperfine 1's runs in any other way than hyperfine 2
    /// summarizes the same runs.
    #[test]
    fn adapter_shell_hyperfine_v1_derives_as_hyperfine_2() {
        let input = std::fs::read_to_string("./tool_output/shell/hyperfine/v2_memory.json")
            .expect("Failed to read a fixture");
        let json: serde_json::Value = serde_json::from_str(&input).expect("Failed to parse");
        let result = &json["results"][0];
        let runs = result["measurements"]
            .as_array()
            .expect("no measurements")
            .iter()
            .map(|run| run["memory_peak_resident"]["value"].clone())
            .collect::<Vec<_>>();
        let results = convert_edited_v1(
            Adapter::ShellHyperfineV1,
            "shell/hyperfine/v1_one_run.json",
            |json| {
                json["results"][0]["memory_usage_byte"] = runs.into();
            },
        );
        let summary = &result["summary"]["memory_peak_resident"];
        for (name, field) in [
            ("mean", "mean"),
            ("std_dev", "stddev"),
            ("median", "median"),
            ("min", "min"),
            ("max", "max"),
        ] {
            assert_value(
                &results,
                ("sleep 0.001", "{}"),
                (PeakMemory::SLUG_STR, name),
                summary[field].as_f64().expect("no statistic"),
            );
        }
    }

    /// Kills the node trying hyperfine 1's format first, or either leaf missing from it.
    #[test]
    fn adapter_shell_hyperfine_v1_node() {
        for (suffix, leaf) in [
            (COUNTERS, Adapter::ShellHyperfineV2),
            ("shell/hyperfine/v2_two.json", Adapter::ShellHyperfineV2),
            ("shell/hyperfine/v1_scan.json", Adapter::ShellHyperfineV1),
            ("shell/hyperfine/two.json", Adapter::ShellHyperfineV1),
        ] {
            let results = convert_v1(Adapter::ShellHyperfine, suffix);
            assert_eq!(results.resolved_adapter, Some(leaf), "{suffix}");
            assert_eq!(Some(results), Some(convert_v1(leaf, suffix)), "{suffix}");
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_shell_hyperfine_v1_ignores_average() {
        for entry in
            std::fs::read_dir("./tool_output/shell/hyperfine").expect("Failed to read the fixtures")
        {
            let path = entry.expect("Failed to read a fixture").path();
            let suffix = path
                .strip_prefix("./tool_output")
                .expect("Failed to strip the prefix")
                .display()
                .to_string();
            assert_ignores_average(Adapter::ShellHyperfine, &suffix);
        }
    }
}
