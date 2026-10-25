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
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::{Finite, finite_json},
            leaf::{LeafResults, Variant},
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// pytest-benchmark's `--benchmark-json` report at version 1.
///
/// A parametrized test's name drops its `[param]` suffix, and its `params` are the parameters,
/// unless a value is not a scalar or there are more than 8, when the full name stays with none.
pub struct AdapterPythonPytestV1;

impl Adaptable for AdapterPythonPytestV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Pytest { benchmarks, .. } = serde_json::from_str(&finite_json(input)).ok()?;
        let mut results = LeafResults::default();
        for benchmark in benchmarks {
            let (name, parameters) = name_and_parameters(&benchmark);
            let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            if let Some(mut variant) = results.new_variant(benchmark_name, parameters) {
                benchmark.stats.insert(&mut variant);
            }
        }
        results.finish_with_skips(Adapter::PythonPytest)
    }
}

#[derive(Deserialize)]
struct Pytest {
    #[serde(rename = "machine_info")]
    _machine_info: IgnoredAny,
    benchmarks: Vec<Benchmark>,
}

#[derive(Deserialize)]
struct Benchmark {
    fullname: String,
    param: Option<String>,
    params: Option<serde_json::Map<String, serde_json::Value>>,
    stats: Stats,
}

/// The statistics of a benchmark's rounds, each the time of one iteration in seconds, and
/// `ops`, the number of rounds over their total time.
#[derive(Deserialize)]
struct Stats {
    mean: Finite,
    median: Finite,
    min: Finite,
    max: Finite,
    stddev: Finite,
    q1: Finite,
    q3: Finite,
    iqr: Finite,
    ld15iqr: Finite,
    hd15iqr: Finite,
    total: Finite,
    ops: Finite,
    rounds: u64,
}

impl Stats {
    /// Every statistic in seconds as latency, the whiskers as pytest-benchmark computes them, and
    /// the rate as throughput.
    ///
    /// `ld15iqr` is the lowest round within 1.5 interquartile ranges below the first quartile, but
    /// `hd15iqr` is the first round past that range above the third quartile when one is.
    fn insert(&self, variant: &mut Variant<'_>) {
        let Self {
            mean,
            median,
            min,
            max,
            stddev,
            q1,
            q3,
            iqr,
            ld15iqr,
            hd15iqr,
            total,
            ops,
            rounds,
        } = *self;
        let latency = Latency::name_id();
        let factor = Units::Sec.as_nanos();
        for (name, value) in [
            ("mean", mean),
            ("median", median),
            ("min", min),
            ("max", max),
            ("p25", q1),
            ("p75", q3),
            ("lower_whisker", ld15iqr),
            ("upper_whisker", hd15iqr),
            ("sum", total),
        ] {
            value.insert(variant, &latency, name, factor);
        }
        iqr.insert_spread(mean, variant, &latency, "iqr", factor);
        // pytest-benchmark writes 0 for what it cannot compute: the deviation of one round, and
        // the rate of rounds that take no time.
        if rounds > 1 {
            stddev.insert_spread(mean, variant, &latency, "std_dev", factor);
        }
        Finite::from(ops.get().filter(|ops| *ops > 0.0)).insert(
            variant,
            &Throughput::name_id(),
            "mean",
            1.0,
        );
    }
}

/// The name without its `[param]` suffix and the `params`, or the full name and no parameters
/// when the `params` cannot be parameters.
fn name_and_parameters(benchmark: &Benchmark) -> (&str, ParameterSet) {
    let Benchmark {
        fullname,
        param,
        params,
        ..
    } = benchmark;
    let Some(params) = params.as_ref().filter(|params| !params.is_empty()) else {
        return (fullname, ParameterSet::default());
    };
    let parameters = params
        .values()
        .all(is_scalar)
        .then(|| serde_json::from_value(serde_json::Value::Object(params.clone())).ok())
        .flatten();
    let Some(parameters) = parameters else {
        return (fullname, ParameterSet::default());
    };
    let name = param
        .as_deref()
        .and_then(|param| {
            fullname
                .strip_suffix(']')?
                .strip_suffix(param)?
                .strip_suffix('[')
        })
        .unwrap_or(fullname);
    (name, parameters)
}

/// A boolean, a number, or a string, but not the string pytest-benchmark writes for a value it
/// cannot serialize.
fn is_scalar(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
        serde_json::Value::String(string) => {
            !(string.starts_with("UNSERIALIZABLE[") && string.ends_with(']'))
        },
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            false
        },
    }
}

#[cfg(test)]
mod test_python_pytest_v1 {
    use std::collections::BTreeSet;

    use bencher_json::{
        ParameterSet,
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
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_value, assert_variant, convert_edited_v1, convert_v1,
            value, variants,
        },
        results::adapter_results::AdapterResults,
    };

    const PARAMS: &str = "python/pytest/params.json";

    fn names(results: &AdapterResults) -> BTreeSet<(String, String)> {
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

    fn canonical(parameters: &str) -> String {
        parameters
            .parse::<ParameterSet>()
            .expect("Failed to parse parameters")
            .canonical()
    }

    /// Kills a statistic left in seconds, `q1` and `q3` misnamed, `ld15iqr` or `hd15iqr` misnamed,
    /// dropped, or swapped, `hd15iqr` corrected rather than stored as computed (it is the first
    /// outlier past the upper fence when one exists, as here), `total` dropped or misnamed, a count stored as a metric, the operations rate put in latency,
    /// and a leaf that records no leaf.
    #[test]
    fn adapter_python_pytest_v1_statistics() {
        let results = convert_v1(Adapter::PythonPytest, PARAMS);
        assert_eq!(results.resolved_adapter, Some(Adapter::PythonPytest));
        assert_variant(
            &results,
            "test_bench.py::test_stacked",
            r#"{"flag":true,"size":16.5,"kind":"list"}"#,
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 184.043_999_979_621_7),
                        ("median", 159.949_995_577_335_36),
                        ("min", 159.339_997_480_856_25),
                        ("max", 278.629_995_591_472_8),
                        ("std_dev", 52.890_238_470_955_29),
                        ("p25", 159.647_504_460_735_92),
                        ("p75", 191.570_002_243_679_5),
                        ("iqr", 31.922_497_782_943_565),
                        ("lower_whisker", 159.339_997_480_856_25),
                        ("upper_whisker", 278.629_995_591_472_8),
                        ("sum", 920.219_999_898_108_6),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 5_433_483.298_073_966)]),
            ],
        );
    }

    /// Kills the standard deviation pytest-benchmark writes as 0 for a single round, where it
    /// computes none, stored.
    #[test]
    fn adapter_python_pytest_v1_one_round() {
        let results = convert_v1(Adapter::PythonPytest, PARAMS);
        let nanos = 1_782.999_788_702;
        assert_variant(
            &results,
            "test_bench.py::test_one_round",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", nanos),
                        ("median", nanos),
                        ("min", nanos),
                        ("max", nanos),
                        ("p25", nanos),
                        ("p75", nanos),
                        ("iqr", 0.0),
                        ("lower_whisker", nanos),
                        ("upper_whisker", nanos),
                        ("sum", nanos),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 560_852.562_258_566_8)]),
            ],
        );
    }

    /// Kills the `[param]` suffix left on the name, a parameter typed as a string, a value that is
    /// not a scalar (`None`, a list, a dict, an object pytest-benchmark writes as
    /// `UNSERIALIZABLE[...]`), a string past 64 characters, or a ninth key kept as a parameter or
    /// dropped with the suffix cut, a fixture's parameter dropped, a class dropped from the name,
    /// and a case that repeats one already read stored over it or left uncounted.
    #[test]
    fn adapter_python_pytest_v1_names() {
        let results = convert_v1(Adapter::PythonPytest, PARAMS);
        let mut expected = BTreeSet::new();
        for (name, parameters) in [
            ("test_bench.py::test_plain", "{}"),
            ("test_bench.py::test_single", r#"{"n":5}"#),
            ("test_bench.py::test_single", r#"{"n":10}"#),
            ("test_bench.py::test_duplicate", r#"{"n":3}"#),
            ("test_bench.py::test_custom_id", r#"{"n":4}"#),
            ("test_bench.py::test_nonscalar[None]", "{}"),
            ("test_bench.py::test_nonscalar[value1]", "{}"),
            ("test_bench.py::test_nonscalar[value2]", "{}"),
            ("test_bench.py::test_nonscalar[value3]", "{}"),
            (
                "test_bench.py::test_nonscalar[xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx]",
                "{}",
            ),
            ("test_bench.py::test_nine[1-2-3-4-5-6-7-8-9]", "{}"),
            (
                "test_bench.py::test_eight",
                r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7,"h":8}"#,
            ),
            ("test_bench.py::test_fixture_param", r#"{"letter":"x"}"#),
            ("test_bench.py::test_fixture_param", r#"{"letter":"y"}"#),
            ("test_bench.py::test_grouped", "{}"),
            ("test_bench.py::TestClass::test_method", r#"{"n":2}"#),
            ("test_bench.py::test_pedantic", "{}"),
            ("test_bench.py::test_one_round", "{}"),
        ] {
            expected.insert((name.to_owned(), canonical(parameters)));
        }
        for flag in ["true", "false"] {
            for size in ["8", "16.5"] {
                for kind in ["list", "tuple"] {
                    expected.insert((
                        "test_bench.py::test_stacked".to_owned(),
                        canonical(&format!(
                            r#"{{"flag":{flag},"size":{size},"kind":"{kind}"}}"#
                        )),
                    ));
                }
            }
        }
        assert_eq!(names(&results), expected);
        assert_eq!(variants(&results), 26);
        assert_eq!(results.skipped.variants, 1);
        assert_value(
            &results,
            ("test_bench.py::test_duplicate", r#"{"n":3}"#),
            (Latency::SLUG_STR, "mean"),
            116.876_002_721_255_65,
        );
    }

    /// Kills a string that ends in `]` read as the value pytest-benchmark could not serialize,
    /// which keeps its case under the full name with no parameters, apart from its siblings.
    #[test]
    fn adapter_python_pytest_v1_bracketed_string() {
        let results = convert_v1(Adapter::PythonPytest, "python/pytest/brackets.json");
        assert!(results.skipped.is_empty());
        let name = "test_index.py::test_index";
        let expected = [r#"{"x":"a[0]"}"#, r#"{"x":"b"}"#]
            .map(|parameters| (name.to_owned(), canonical(parameters)));
        assert_eq!(names(&results), expected.into_iter().collect());
    }

    /// Kills a case parametrized with the empty string kept under its full name with no
    /// parameters, apart from its siblings, or stored as if its key were unset.
    #[test]
    fn adapter_python_pytest_v1_empty_string() {
        let results = convert_v1(Adapter::PythonPytest, "python/pytest/empty.json");
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 2);
        for (parameters, median) in [
            (r#"{"s":""}"#, 40.298_390_001_313_94),
            (r#"{"s":"a"}"#, 45.558_556_086_836_84),
        ] {
            assert_value(
                &results,
                ("test_empty.py::test_join", parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a trailing `[...]` cut from the name when it is not the benchmark's `param`.
    #[test]
    fn adapter_python_pytest_v1_suffix_not_param() {
        let results = convert_edited_v1(Adapter::PythonPytest, PARAMS, |json| {
            json["benchmarks"][1]["param"] = "6".into();
        });
        assert!(
            results
                .inner
                .keys()
                .any(|name| name.to_string() == "test_bench.py::test_single[5]")
        );
    }

    /// Kills the rate of 0 pytest-benchmark writes when its samples sum to 0, for an infinite
    /// rate, stored or left uncounted.
    #[test]
    fn adapter_python_pytest_v1_zero_rate() {
        let results = convert_edited_v1(Adapter::PythonPytest, PARAMS, |json| {
            json["benchmarks"][0]["stats"]["ops"] = 0.into();
        });
        assert_eq!(results.skipped.metrics, 1);
        assert_eq!(
            value(
                &results,
                "test_bench.py::test_plain",
                "{}",
                Throughput::SLUG_STR,
                "mean"
            ),
            None
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_python_pytest_v1_ignores_average() {
        for suffix in [PARAMS, "python/pytest/two.json", "python/pytest/four.json"] {
            assert_ignores_average(Adapter::PythonPytest, suffix);
        }
    }
}
