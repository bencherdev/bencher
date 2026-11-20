use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
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
    adapters::bmf_v1::{
        finite::{Finite, finite_json},
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// kotlinx-benchmark's JSON report for a JS, Wasm, or Native target: each benchmark on a variant
/// of its `@Param` values and `@mode`, with the statistics kotlinx computes over its iterations.
///
/// A JVM target's report is JMH's own, which `java_jmh` reads. kotlinx writes neither of the keys
/// JMH always writes, `jmhVersion` and `forks` (`BenchmarkReportFormatter.kt:168-207`, v0.5.0).
pub struct AdapterKotlinKotlinx;

impl Adaptable for AdapterKotlinKotlinx {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let benchmarks = serde_json::from_str::<Vec<Benchmark>>(&finite_json(input)).ok()?;
        if benchmarks.iter().any(Benchmark::is_jmh) {
            return None;
        }
        let mut results = LeafResults::default();
        for benchmark in benchmarks {
            let Benchmark {
                name,
                mode,
                mut params,
                primary_metric,
                ..
            } = benchmark;
            let (Some((measure, factor)), Ok(benchmark_name)) = (
                primary_measure(&mode, &primary_metric.score_unit),
                name.parse::<BenchmarkName>(),
            ) else {
                results.skip_benchmark();
                continue;
            };
            if params.remove(MODE).is_some() {
                results.skip_variant();
            }
            let mut parameters = params
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect::<serde_json::Map<_, _>>();
            parameters.insert(MODE.to_owned(), mode.into());
            let Some(parameters) = parameter_set(parameters) else {
                results.skip_variant();
                continue;
            };
            if let Some(mut variant) = results.new_variant(benchmark_name, parameters) {
                primary_metric.insert(&mut variant, &measure, factor);
            }
        }
        results.finish_with_skips(Adapter::KotlinKotlinx)
    }
}

const MODE: &str = "@mode";

/// The measure of a mode's score and the factor from its unit to the measure's: a throughput per
/// second, or a time in nanoseconds. kotlinx has only these 2 modes, and its unit text is `ops/`
/// and a time unit, or a time unit and `/op` (`ReportBenchmarksStatistics.kt:128-132`).
fn primary_measure(mode: &str, unit: &str) -> Option<(MeasureNameId, f64)> {
    match mode {
        "thrpt" => Some((
            Throughput::name_id(),
            NANOS_PER_SECOND / nanos(unit.strip_prefix("ops/")?)?,
        )),
        "avgt" => Some((Latency::name_id(), nanos(unit.strip_suffix("/op")?)?)),
        _ => None,
    }
}

/// The nanoseconds in one of kotlinx's time units, as it spells them
/// (`CommonBenchmarkAnnotations.kt:37-44`).
fn nanos(unit: &str) -> Option<f64> {
    Some(match unit {
        "ns" => 1.0,
        "us" => 1_000.0,
        "ms" => 1_000_000.0,
        "sec" => NANOS_PER_SECOND,
        "min" => 60.0 * NANOS_PER_SECOND,
        _ => return None,
    })
}

const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Benchmark {
    #[serde(rename = "benchmark")]
    name: String,
    mode: String,
    params: BTreeMap<String, String>,
    primary_metric: Metric,
    jmh_version: Option<IgnoredAny>,
    forks: Option<IgnoredAny>,
}

impl Benchmark {
    fn is_jmh(&self) -> bool {
        self.jmh_version.is_some() || self.forks.is_some()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    score: Finite,
    score_confidence: [Finite; 2],
    score_percentiles: BTreeMap<String, Finite>,
    score_unit: String,
    raw_data: Vec<Vec<Finite>>,
}

impl Metric {
    /// The score as `mean` with its 95% interval, which kotlinx fixes at `1.96` standard errors
    /// (`ReportBenchmarksStatistics.kt:65`), and the percentiles of the iterations.
    ///
    /// kotlinx writes the score itself as both bounds of one iteration, since its standard
    /// deviation of one value is 0 (`:46-47`), so that interval is skipped uncounted.
    fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId, factor: f64) {
        let Self {
            score,
            score_confidence: [lower, upper],
            score_percentiles,
            raw_data,
            ..
        } = self;
        score.insert(variant, measure, "mean", factor);
        if raw_data.iter().map(Vec::len).sum::<usize>() > 1 {
            lower.insert_spread(*score, variant, measure, "mean_ci95_lower", factor);
            upper.insert_spread(*score, variant, measure, "mean_ci95_upper", factor);
        }
        for (percent, value) in score_percentiles {
            if let Some(name) = percentile_name(percent) {
                value.insert(variant, measure, &name, factor);
            } else {
                variant.skip_metric();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test_kotlin_kotlinx {
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
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
            convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    /// The percentiles kotlinx writes, in its order.
    const PERCENTILES: [&str; 9] = [
        "min", "p25", "median", "p75", "p90", "p99", "p99_9", "p99_99", "max",
    ];

    /// The quoted `@Param` value of the parameter benchmarks, in a variant with its `@mode`.
    const QUOTED: &str =
        r#"{"@mode":"thrpt","data":"1","text":"a \"string\" with quotes","value":"2"}"#;
    const MATH: &str = "test.ParamBenchmark.mathBenchmark";

    /// Kills a leaf that names kotlinx's 95% interval as JMH's 99.9%, names a percentile
    /// inaccurately, drops or renames a `@Param` key, reads its value as anything but its text,
    /// leaves out `@mode`, scales operations per millisecond wrongly, or records another leaf.
    #[test]
    fn adapter_kotlin_kotlinx_params() {
        let results = convert_v1(
            Adapter::KotlinKotlinx,
            "kotlin/kotlinx/params_linuxX64.json",
        );
        assert_eq!(results.resolved_adapter, Some(Adapter::KotlinKotlinx));
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(results.inner.len(), 2);
        assert_eq!(variants(&results), 8);
        assert_variant(
            &results,
            MATH,
            QUOTED,
            &[scored(
                Throughput::SLUG_STR,
                [
                    162_548_045.278_953_22,
                    160_743_233.744_483_7,
                    164_352_856.813_422_7,
                ],
                rising(
                    [160_895_210.458_428_5, 162_671_011.605_328_1],
                    164_077_913.773_102_97,
                ),
            )],
        );
        assert_units(&results, &[]);
    }

    /// Kills a leaf that refuses a target's report by a key only some targets or versions write:
    /// `compilationMode` differs between Native and JS or Wasm, and 0.6.0 adds `threads`.
    #[test]
    fn adapter_kotlin_kotlinx_every_target() {
        for (name, mean) in [
            ("params_js.json", 175_421_835.975_634_55),
            ("params_wasmJs.json", 61_360_116.764_510_03),
            ("params_wasmWasi.json", 49_469_351.501_046_98),
            ("v0_6_0_rc_params_linuxX64.json", 142_639_774.302_492_4),
        ] {
            let results = convert_v1(Adapter::KotlinKotlinx, &format!("kotlin/kotlinx/{name}"));
            assert!(results.skipped.is_empty(), "{name} {:?}", results.skipped);
            assert_eq!(variants(&results), 8, "{name}");
            assert_value(
                &results,
                (MATH, QUOTED),
                (Throughput::SLUG_STR, "mean"),
                mean,
            );
        }
    }

    /// Kills a leaf that drops or misscales a unit: seconds, which kotlinx spells `sec` where JMH
    /// writes `s`, minutes, milliseconds, microseconds, nanoseconds, or operations per second.
    #[test]
    fn adapter_kotlin_kotlinx_units() {
        let results = convert_v1(
            Adapter::KotlinKotlinx,
            "kotlin/kotlinx/avgt_sec_linuxX64.json",
        );
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_variant(
            &results,
            "test.DefaultBenchmark.mathBenchmark",
            r#"{"@mode":"avgt"}"#,
            &[scored(
                Latency::SLUG_STR,
                [
                    13.282_799_686_655_185,
                    12.995_596_822_811_69,
                    13.570_002_550_498_678,
                ],
                rising(
                    [12.993_358_352_088_187, 13.387_736_802_330_416],
                    13.467_303_905_546_945,
                ),
            )],
        );

        let results = convert_v1(
            Adapter::KotlinKotlinx,
            "kotlin/kotlinx/thrpt_min_linuxX64.json",
        );
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_variant(
            &results,
            "test.DefaultBenchmark.mathBenchmark",
            r#"{"@mode":"thrpt"}"#,
            &[scored(
                Throughput::SLUG_STR,
                [
                    73_975_341.590_683_09,
                    70_412_510.668_741_46,
                    77_538_172.512_624_73,
                ],
                rising(
                    [71_527_128.717_100_44, 72_871_882.089_667_19],
                    77_527_013.965_281_64,
                ),
            )],
        );

        let results = convert_v1(Adapter::KotlinKotlinx, "kotlin/kotlinx/main_linuxX64.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(results.inner.len(), 49);
        assert_eq!(variants(&results), 143);
        for (benchmark, parameters, measure, mean) in [
            (
                "test.CommonBenchmark.longBenchmark",
                r#"{"@mode":"avgt"}"#,
                Latency::SLUG_STR,
                651.146_220_027_408,
            ),
            (
                "test.nested.DefaultBenchmark.sum",
                r#"{"@mode":"avgt"}"#,
                Latency::SLUG_STR,
                260.872_219_116_375_85,
            ),
            (
                "test.ManyBenchmark.case00",
                r#"{"@mode":"avgt","n":"1"}"#,
                Latency::SLUG_STR,
                2.711_998_236_303_923,
            ),
            (
                "test.DefaultBenchmark.mathBenchmark",
                r#"{"@mode":"thrpt"}"#,
                Throughput::SLUG_STR,
                69_800_889.104_130_9,
            ),
        ] {
            assert_value(&results, (benchmark, parameters), (measure, "mean"), mean);
        }

        let results = convert_v1(Adapter::KotlinKotlinx, "kotlin/kotlinx/avgt_sec_js.json");
        assert_value(
            &results,
            ("test.DefaultBenchmark.sum", r#"{"@mode":"avgt"}"#),
            (Latency::SLUG_STR, "mean"),
            39.870_252_361_947_735,
        );
    }

    /// Kills a leaf that writes the zero-width interval of one iteration, counts its skip, or
    /// drops the percentiles beside it.
    #[test]
    fn adapter_kotlin_kotlinx_one_iteration() {
        for name in [
            "one_linuxX64.json",
            "one_js.json",
            "v0_6_0_rc_one_linuxX64.json",
        ] {
            let results = convert_v1(Adapter::KotlinKotlinx, &format!("kotlin/kotlinx/{name}"));
            assert!(results.skipped.is_empty(), "{name} {:?}", results.skipped);
            assert_eq!(variants(&results), 2, "{name}");
        }
        let results = convert_v1(Adapter::KotlinKotlinx, "kotlin/kotlinx/one_linuxX64.json");
        let mut metrics: Vec<_> = PERCENTILES.map(|name| (name, 263.919_071_602_795_6)).into();
        metrics.push(("mean", 263.919_071_602_795_6));
        assert_variant(
            &results,
            "test.nested.DefaultBenchmark.sum",
            r#"{"@mode":"avgt"}"#,
            &[(Latency::SLUG_STR, metrics)],
        );
    }

    /// kotlinx writes a zero-width interval for several equal iterations too, which is a real
    /// interval, so only the iteration count decides the skip. The edit sets each iteration of a
    /// real benchmark to its mean, as kotlinx writes it then (`ReportBenchmarksStatistics.kt:44-53`).
    ///
    /// Kills: a skip keyed on a zero-width interval or a `scoreError` of 0.
    #[test]
    fn adapter_kotlin_kotlinx_equal_iterations() {
        let results = edited("params_linuxX64.json", |json| {
            let metric = &mut json[0]["primaryMetric"];
            let score = metric["score"].clone();
            metric["scoreError"] = 0.0.into();
            metric["scoreConfidence"] = serde_json::json!([score, score]);
            metric["rawData"] = serde_json::json!([[score, score, score]]);
            for percentile in metric["scorePercentiles"]
                .as_object_mut()
                .expect("Failed to read the percentiles")
                .values_mut()
            {
                *percentile = score.clone();
            }
        })
        .expect("expected the leaf to read the edited report");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        for name in ["mean_ci95_lower", "mean_ci95_upper"] {
            assert_value(
                &results,
                (MATH, QUOTED),
                (Throughput::SLUG_STR, name),
                162_548_045.278_953_22,
            );
        }
    }

    /// kotlinx 0.4.0 to 0.4.7 write no `advanced`, `compilationMode`, or `configurationName`, and
    /// write `nativeFork` and `nativeGCAfterIteration` at their defaults, `perBenchmark` and
    /// `false` (`BenchmarkReportFormatter.kt:165-202`, `ExecutorConfiguration.kt:20-21`, v0.4.7),
    /// so the edit turns a real report into that shape.
    ///
    /// Kills: a claim keyed on any key that only newer releases write.
    #[test]
    fn adapter_kotlin_kotlinx_before_0_4_8() {
        let results = edited("params_linuxX64.json", |json| {
            each_benchmark(json, |benchmark| {
                let benchmark = benchmark
                    .as_object_mut()
                    .expect("Failed to read a benchmark");
                for key in ["advanced", "compilationMode", "configurationName"] {
                    assert!(benchmark.remove(key).is_some(), "no {key}");
                }
                benchmark.insert("nativeFork".to_owned(), "perBenchmark".into());
                benchmark.insert("nativeGCAfterIteration".to_owned(), false.into());
            });
        });
        assert_eq!(
            results,
            Some(convert_v1(
                Adapter::KotlinKotlinx,
                "kotlin/kotlinx/params_linuxX64.json"
            ))
        );
    }

    /// JMH writes `jmhVersion` since 1.19 and `forks` in every release, and Swift package-benchmark
    /// writes `forks`, so either key on any benchmark refuses the report.
    ///
    /// Kills: a claim that checks one key, or the first benchmark only.
    #[test]
    fn adapter_kotlin_kotlinx_refuses_a_jmh_key() {
        for key in ["jmhVersion", "forks"] {
            let results = edited("params_linuxX64.json", |json| {
                json[7][key] = serde_json::json!(1);
            });
            assert_eq!(results, None, "{key}");
        }
    }

    /// Kills a leaf that lets a `@Param` spelled like `@mode` replace the mode, or drops it
    /// uncounted.
    #[test]
    fn adapter_kotlin_kotlinx_adapter_key() {
        let results = edited("params_linuxX64.json", |json| {
            json[0]["params"]["@mode"] = "fast".into();
        })
        .expect("expected the leaf to read the edited report");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 8);
        assert_value(
            &results,
            (MATH, QUOTED),
            (Throughput::SLUG_STR, "mean"),
            162_548_045.278_953_22,
        );
    }

    /// Kills a leaf that lets a second benchmark of one name, parameters, and mode overwrite the
    /// first, or drops it uncounted.
    #[test]
    fn adapter_kotlin_kotlinx_repeated_benchmark() {
        let results = edited("params_linuxX64.json", |json| {
            let mut repeat = json[0].clone();
            repeat["primaryMetric"]["score"] = 1.0.into();
            json.as_array_mut()
                .expect("Failed to read the benchmarks")
                .push(repeat);
        })
        .expect("expected the leaf to read the edited report");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_value(
            &results,
            (MATH, QUOTED),
            (Throughput::SLUG_STR, "mean"),
            162_548_045.278_953_22,
        );
    }

    /// Kills: a leaf that refuses the report, or keeps or drops uncounted what is not finite.
    #[test]
    fn adapter_kotlin_kotlinx_not_finite() {
        let results = Adapter::KotlinKotlinx
            .convert(&not_finite(), Settings::new(None, BmfVersion::V1))
            .expect("expected the leaf to read the edited report");
        assert_eq!(
            results.skipped,
            Skipped {
                metrics: 4,
                ..Skipped::default()
            }
        );
        let metric = |name| value(&results, MATH, QUOTED, Throughput::SLUG_STR, name);
        assert_eq!(metric("mean"), None);
        assert_eq!(metric("max"), None);
        assert_eq!(metric("median"), Some(162_671_011.605_328_1));
    }

    /// kotlinx writes each number with Kotlin's `Double.toString` (`BenchmarkReportFormatter.kt:185-190`),
    /// which prints an infinite iteration rate, from an iteration timed at 0, as a bare
    /// `Infinity` that JSON has no token for, and its interval as `NaN`. The edit puts those in a
    /// real report's text.
    pub(crate) fn not_finite() -> String {
        let mut edited =
            std::fs::read_to_string("./tool_output/kotlin/kotlinx/params_linuxX64.json")
                .expect("Failed to read a fixture");
        for (from, to) in [
            (r#""score": 162548.0452789532"#, r#""score": Infinity"#),
            ("160743.2337444837,", "NaN,"),
            ("164352.85681342272\n", "NaN\n"),
            (r#""100.00" : 164077.91377310298"#, r#""100.00" : Infinity"#),
        ] {
            assert_eq!(edited.matches(from).count(), 1, "{from}");
            edited = edited.replace(from, to);
        }
        edited
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_kotlin_kotlinx_ignores_average() {
        for name in ["main_linuxX64.json", "one_js.json", "params_linuxX64.json"] {
            assert_ignores_average(Adapter::KotlinKotlinx, &format!("kotlin/kotlinx/{name}"));
        }
    }

    /// A mean with its 95% interval, in the order mean, lower, upper, and the percentiles.
    fn scored(
        measure: &'static str,
        [mean, lower, upper]: [f64; 3],
        percentiles: [f64; 9],
    ) -> Expected {
        let mut metrics: Vec<_> = PERCENTILES.into_iter().zip(percentiles).collect();
        metrics.extend([
            ("mean", mean),
            ("mean_ci95_lower", lower),
            ("mean_ci95_upper", upper),
        ]);
        (measure, metrics)
    }

    /// The minimum and the median, then one value for every higher percentile, where the lower
    /// quartile of 3 iterations is the minimum.
    fn rising([min, median]: [f64; 2], rest: f64) -> [f64; 9] {
        let mut percentiles = [rest; 9];
        percentiles[0] = min;
        percentiles[1] = min;
        percentiles[2] = median;
        percentiles
    }

    /// The fixture at `kotlin/kotlinx/<name>` after `edit`, through the leaf at version 1.
    fn edited<F: FnOnce(&mut serde_json::Value)>(name: &str, edit: F) -> Option<AdapterResults> {
        let input = std::fs::read_to_string(format!("./tool_output/kotlin/kotlinx/{name}"))
            .expect("Failed to read a fixture");
        let mut json =
            serde_json::from_str::<serde_json::Value>(&input).expect("Failed to parse a fixture");
        edit(&mut json);
        Adapter::KotlinKotlinx.convert(&json.to_string(), Settings::new(None, BmfVersion::V1))
    }

    /// Every benchmark of the fixture after `edit`.
    fn each_benchmark(json: &mut serde_json::Value, edit: impl Fn(&mut serde_json::Value)) {
        for benchmark in json.as_array_mut().expect("Failed to read the benchmarks") {
            edit(benchmark);
        }
    }
}
