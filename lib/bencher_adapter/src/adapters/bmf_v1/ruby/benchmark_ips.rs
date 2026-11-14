use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{BuiltInMeasure as _, default::Throughput},
        report::Adapter,
    },
};
use serde::{Deserialize, Deserializer, de::Error as _};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::leaf::{LeafResults, Variant},
    results::adapter_results::AdapterResults,
};

/// benchmark-ips's `json!` export at version 1: each entry is one benchmark, named by its label,
/// with its rate as throughput.
///
/// The export names no statistics mode, so the JSON type of `error` tells it, and each mode is its
/// own variant, `@stats`, an adapter key that starts with `@`, following JSON-LD's convention for
/// keywords.
pub struct AdapterRubyBenchmarkIps;

impl Adaptable for AdapterRubyBenchmarkIps {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let entries: Vec<Entry> = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for Entry {
            name,
            central_tendency,
            error,
        } in entries
        {
            let Ok(benchmark_name) = label(name).parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            let Some(parameters) = error.parameters() else {
                results.skip_variant();
                continue;
            };
            if let Some(mut variant) = results.new_variant(benchmark_name, parameters) {
                error.insert(&mut variant, central_tendency);
            }
        }
        results.finish_with_skips(Adapter::RubyBenchmarkIps)
    }
}

#[derive(Deserialize)]
struct Entry {
    name: serde_json::Value,
    /// Iterations per second.
    central_tendency: f64,
    error: Spread,
}

/// A label as written, or the compact JSON text of a label that is not a string, its object keys
/// sorted, as the parsed map keeps them.
fn label(name: serde_json::Value) -> String {
    if let serde_json::Value::String(label) = name {
        label
    } else {
        name.to_string()
    }
}

/// `error`, an integer in the default mode and a float in bootstrap mode.
#[derive(Clone, Copy)]
enum Spread {
    /// The population standard deviation of the per-sample rates about the reported rate.
    StdDev(f64),
    /// The half-width of the rate's bootstrap confidence interval, at a level the export omits.
    Margin(f64),
}

impl Spread {
    fn parameters(self) -> Option<ParameterSet> {
        let stats = match self {
            Self::StdDev(_) => "sd",
            Self::Margin(_) => "bootstrap",
        };
        let mut parameters = serde_json::Map::new();
        parameters.insert(STATS.to_owned(), stats.into());
        serde_json::from_value(serde_json::Value::Object(parameters)).ok()
    }

    fn insert(self, variant: &mut Variant<'_>, mean: f64) {
        let throughput = Throughput::name_id();
        variant.insert(throughput.clone(), "mean", mean);
        match self {
            Self::StdDev(std_dev) => variant.insert(throughput, "std_dev", std_dev),
            Self::Margin(margin) => {
                variant.insert(throughput.clone(), "mean_ci_lower", mean - margin);
                variant.insert(throughput, "mean_ci_upper", mean + margin);
            },
        }
    }
}

/// The gem's name for the setting, `x.config(stats: :bootstrap)`.
const STATS: &str = "@stats";

impl<'de> Deserialize<'de> for Spread {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let number = serde_json::Number::deserialize(deserializer)?;
        let value = number
            .as_f64()
            .ok_or_else(|| D::Error::custom(format!("expected a number, found {number}")))?;
        Ok(if number.is_f64() {
            Self::Margin(value)
        } else {
            Self::StdDev(value)
        })
    }
}

#[cfg(test)]
mod test_ruby_benchmark_ips {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Throughput},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_edited_v1,
            convert_v1, variants,
        },
        results::adapter_results::AdapterResults,
    };

    const SD: &str = r#"{"@stats":"sd"}"#;
    const BOOTSTRAP: &str = r#"{"@stats":"bootstrap"}"#;

    /// The default mode's rate and its standard deviation.
    fn sd(mean: f64, std_dev: f64) -> Vec<Expected> {
        vec![(
            Throughput::SLUG_STR,
            vec![("mean", mean), ("std_dev", std_dev)],
        )]
    }

    /// Bootstrap mode's rate and its interval, the rate minus and plus `error`.
    fn bootstrap(mean: f64, error: f64) -> Vec<Expected> {
        vec![(
            Throughput::SLUG_STR,
            vec![
                ("mean", mean),
                ("mean_ci_lower", mean - error),
                ("mean_ci_upper", mean + error),
            ],
        )]
    }

    fn assert_sd(results: &AdapterResults, expected: &[(&str, f64, f64)]) {
        for (name, mean, std_dev) in expected {
            assert_variant(results, name, SD, &sd(*mean, *std_dev));
        }
        assert_eq!(variants(results), expected.len());
    }

    fn assert_bootstrap(results: &AdapterResults, expected: &[(&str, f64, f64)]) {
        for (name, mean, error) in expected {
            assert_variant(results, name, BOOTSTRAP, &bootstrap(*mean, *error));
        }
        assert_eq!(variants(results), expected.len());
    }

    /// Kills the rate read into latency or from another field, `error` stored as an interval or
    /// dropped in the default mode, `@stats` missing or misnamed, `ips`, `stddev`,
    /// `microseconds`, `iterations`, or `cycles` written, a measure given a unit of the leaf's own,
    /// a leaf that records no leaf, and the export read by a version 0 project.
    #[test]
    fn adapter_ruby_benchmark_ips_plain() {
        let results = convert_v1(Adapter::RubyBenchmarkIps, "ruby/benchmark_ips/plain.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::RubyBenchmarkIps));
        assert!(results.skipped.is_empty());
        assert_units(&results, &[]);
        assert_sd(
            &results,
            &[
                ("addition", 52_262_003.661_397_42, 726_873.0),
                ("addition2", 145_552_932.100_528_48, 9_055_758.0),
                ("addition3", 142_928_367.151_789_22, 10_228_038.0),
                ("symbol_label", 51_406_516.853_136_87, 922_510.0),
                (
                    "addition-test-long-label",
                    50_655_311.145_422_84,
                    2_906_285.0,
                ),
            ],
        );
        let input = std::fs::read_to_string("./tool_output/ruby/benchmark_ips/plain.json")
            .expect("Failed to read a fixture");
        assert_eq!(
            Adapter::RubyBenchmarkIps.convert(&input, Settings::new(None, BmfVersion::V0)),
            None
        );
    }

    /// Kills benchmark-ips 2.14.0's export, which has the same fields, refused.
    #[test]
    fn adapter_ruby_benchmark_ips_2_14() {
        let results = convert_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/plain_2_14.json",
        );
        assert!(results.skipped.is_empty());
        assert_sd(
            &results,
            &[
                ("addition", 49_916_551.921_250_73, 3_573_701.0),
                ("addition2", 142_626_436.074_637_9, 7_960_401.0),
                ("addition3", 141_308_105.939_646, 11_307_635.0),
                ("symbol_label", 51_372_264.108_172_53, 1_158_502.0),
                (
                    "addition-test-long-label",
                    49_583_989.925_557_32,
                    1_322_711.0,
                ),
            ],
        );
    }

    /// Kills a float `error` read as a standard deviation, the interval's bounds swapped or
    /// taken at another width than `error`, a level put in the interval's names (the export
    /// states none, so 95% and 99% runs share them), and bootstrap runs given the default
    /// mode's `@stats`.
    #[test]
    fn adapter_ruby_benchmark_ips_bootstrap() {
        for (suffix, expected) in [
            (
                "ruby/benchmark_ips/bootstrap_95.json",
                [
                    ("mul", 30_854_068.954_963_49, 481_015.084_346_741_44),
                    ("pow", 33_988_293.364_170_924, 518_145.876_829_965_04),
                ],
            ),
            (
                "ruby/benchmark_ips/bootstrap_99.json",
                [
                    ("mul", 33_140_712.004_574_575, 640_793.452_132_068_6),
                    ("pow", 34_825_505.932_373_18, 582_456.503_515_381_4),
                ],
            ),
        ] {
            let results = convert_v1(Adapter::RubyBenchmarkIps, suffix);
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_bootstrap(&results, &expected);
        }
    }

    /// Kills the mode read from `error`'s value rather than its JSON type: a single sample's
    /// `error` is `0` in the default mode, a standard deviation of 0, and `0.0` in bootstrap mode,
    /// an interval of no width.
    #[test]
    fn adapter_ruby_benchmark_ips_one_sample() {
        let results = convert_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/fast_slow.json",
        );
        assert!(results.skipped.is_empty());
        assert_sd(
            &results,
            &[
                ("empty", 57_319_032.361_036_09, 2_120_663.0),
                ("sleep 1.5s", 0.665_972_966_138_117_5, 0.0),
                ("variable sleep", 131.512_076_041_467_6, 562.0),
            ],
        );
        let results = convert_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/bootstrap_one.json",
        );
        assert!(results.skipped.is_empty());
        assert_bootstrap(&results, &[("sleep 1.5s", 0.665_848_788_175_885_2, 0.0)]);
    }

    /// Kills a label that is not a string skipped, named by Ruby's `inspect` or by its JSON with
    /// its keys as written, or made parameters, an interpolated label split into parameters, an
    /// empty label stored or left uncounted, and a label cut at a space or a slash.
    #[test]
    fn adapter_ruby_benchmark_ips_labels() {
        let results = convert_v1(Adapter::RubyBenchmarkIps, "ruby/benchmark_ips/labels.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
        assert_sd(
            &results,
            &[
                ("sort/10", 6_647_471.614_468_083, 67_745.0),
                ("sort_by size=10", 1_732_466.727_069_207_2, 3_569.0),
                ("sort/1000", 58_960.011_283_714_5, 1_371.0),
                ("sort_by size=1000", 13_070.988_990_901_307, 115.0),
                (
                    r#"{"method":"inspect","version":"3.2.3"}"#,
                    10_700_651.129_657_98,
                    106_620.0,
                ),
                (
                    r#"{"fast":true,"n":3,"none":null,"ratio":0.5}"#,
                    10_589_144.227_410_272,
                    104_296.0,
                ),
                ("42", 55_154_350.478_919, 330_101.0),
                (r#"["array",1]"#, 52_313_632.080_370_076, 214_206.0),
                ("repeated", 1_171_078.618_366_630_5, 50_144.0),
                ("threads x4", 17_683.760_897_991_8, 3_258.0),
                ("unicode ✓ label", 54_680_543.800_896_965, 795_177.0),
            ],
        );
    }

    /// Kills an export whose only benchmark is skipped read as a failed parse, which drops its
    /// count: an unlabeled `x.report { }` is labeled `""` by the gem.
    #[test]
    fn adapter_ruby_benchmark_ips_unlabeled() {
        let results = convert_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/unlabeled.json",
        );
        assert_eq!(results.resolved_adapter, Some(Adapter::RubyBenchmarkIps));
        assert_eq!(variants(&results), 0);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills a label past the name limit stored cut to it, or left uncounted.
    #[test]
    fn adapter_ruby_benchmark_ips_long_label() {
        let results = convert_edited_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/labels.json",
            |json| json[0]["name"] = "x".repeat(1025).into(),
        );
        assert_eq!(results.skipped.benchmarks, 2);
        assert_eq!(variants(&results), 10);
    }

    /// Kills a label that repeats one already read stored over it or left uncounted: a Symbol and
    /// a String that print alike are both kept by the export.
    #[test]
    fn adapter_ruby_benchmark_ips_dup_symbol() {
        let results = convert_v1(
            Adapter::RubyBenchmarkIps,
            "ruby/benchmark_ips/dup_symbol.json",
        );
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(results.skipped.benchmarks, 0);
        assert_sd(&results, &[("dup", 48_259_682.347_551_174, 3_626_397.0)]);
    }

    /// Kills an entry dropped from the export of the last `hold!` run, which holds every run's
    /// results, or of a `save!` run, which repeats the saved ones, and from a custom suite's run.
    #[test]
    fn adapter_ruby_benchmark_ips_hold_save_suite() {
        for (suffix, expected) in [
            (
                "ruby/benchmark_ips/hold_2.json",
                vec![
                    ("without", 9_225_210.564_780_684, 337_150.0),
                    ("with", 5_895_584.004_048_170_5, 529_972.0),
                ],
            ),
            (
                "ruby/benchmark_ips/save_2.json",
                vec![
                    ("without", 10_312_916.300_229_339, 683_346.0),
                    ("with", 9_462_207.997_138_573, 892_692.0),
                ],
            ),
            (
                "ruby/benchmark_ips/suite.json",
                vec![("job1", 10_268_390.235_883_243, 2_793_410.0)],
            ),
        ] {
            let results = convert_v1(Adapter::RubyBenchmarkIps, suffix);
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_sd(&results, &expected);
        }
    }

    /// Kills an export with no benchmark read as an empty report rather than a failed parse.
    #[test]
    fn adapter_ruby_benchmark_ips_empty() {
        let input = std::fs::read_to_string("./tool_output/ruby/benchmark_ips/empty.json")
            .expect("Failed to read a fixture");
        assert_eq!(
            Adapter::RubyBenchmarkIps.convert(&input, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_ruby_benchmark_ips_ignores_average() {
        for suffix in [
            "ruby/benchmark_ips/bootstrap_95.json",
            "ruby/benchmark_ips/bootstrap_99.json",
            "ruby/benchmark_ips/bootstrap_one.json",
            "ruby/benchmark_ips/dup_symbol.json",
            "ruby/benchmark_ips/fast_slow.json",
            "ruby/benchmark_ips/hold_2.json",
            "ruby/benchmark_ips/labels.json",
            "ruby/benchmark_ips/plain.json",
            "ruby/benchmark_ips/plain_2_14.json",
            "ruby/benchmark_ips/save_2.json",
            "ruby/benchmark_ips/suite.json",
        ] {
            assert_ignores_average(Adapter::RubyBenchmarkIps, suffix);
        }
    }
}
