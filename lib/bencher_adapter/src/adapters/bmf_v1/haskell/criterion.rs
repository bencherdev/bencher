use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{CpuTime, Cycles, Latency},
            dotnet::Allocated,
            haskell::{GcCollections, GcCopied},
        },
        report::Adapter,
    },
};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, de::Error as _};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant},
    },
    results::adapter_results::AdapterResults,
};

/// criterion's `--json` report, which `bench` writes too.
///
/// Each benchmark gives its OLS slope on its iterations for each responder with a measure, and the
/// mean and standard deviation of its time, each with its bootstrap interval at the level the
/// report states. Raw measurements stay unread.
pub struct AdapterHaskellCriterion;

impl Adaptable for AdapterHaskellCriterion {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let (Header, _version, reports): (Header, String, Vec<Report>) =
            serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for report in reports {
            report.insert(&mut results);
        }
        results.finish_with_skips(Adapter::HaskellCriterion)
    }
}

/// The report file's first element, criterion's `headerRoot` (`Criterion/IO.hs:50-51`), which
/// criterion before 1.3.0.0 misspelled `criterio` (`Criterion/IO.hs:55` at 1.2.6.0).
struct Header;

impl<'de> Deserialize<'de> for Header {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let root = String::deserialize(deserializer)?;
        if root == "criterion" || root == "criterio" {
            Ok(Self)
        } else {
            Err(D::Error::custom(format!(
                "expected a criterion report, found {root}"
            )))
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    report_name: String,
    report_analysis: Analysis,
}

impl Report {
    /// The benchmark under its name, or counted as skipped when the name is not valid or an
    /// earlier report already gave it.
    fn insert(self, results: &mut LeafResults) {
        let Ok(name) = self.report_name.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return;
        };
        let Some(mut variant) = results.new_variant(name, ParameterSet::default()) else {
            return;
        };
        let Analysis {
            regressions,
            mean,
            std_dev,
        } = self.report_analysis;
        let mut responders = Vec::new();
        for regression in regressions {
            regression.insert(&mut variant, &mut responders);
        }
        let latency = Latency::name_id();
        mean.insert(&mut variant, &latency, "mean", Scale::Seconds);
        std_dev.insert(&mut variant, &latency, "std_dev", Scale::Seconds);
    }
}

/// The mean and standard deviation are of the time per iteration (`Criterion/Analysis.hs:152-162`).
#[derive(Deserialize)]
struct Analysis {
    #[serde(rename = "anRegress")]
    regressions: Vec<Regression>,
    #[serde(rename = "anMean")]
    mean: Estimate,
    #[serde(rename = "anStdDev")]
    std_dev: Estimate,
}

/// One `--regress` regression, the first always `time` on `iters` (`Criterion/Analysis.hs:160`),
/// with a coefficient for each predictor and `y` for the intercept (`Criterion/Analysis.hs:208`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Regression {
    reg_responder: String,
    reg_coeffs: BTreeMap<String, Estimate>,
}

impl Regression {
    /// The slope on iterations alone under the responder's measure, or counted as skipped when
    /// the responder has no measure, the regression has another predictor, or an earlier
    /// regression already gave the responder.
    fn insert(mut self, variant: &mut Variant<'_>, responders: &mut Vec<String>) {
        let slope = self.reg_coeffs.remove(ITERS);
        self.reg_coeffs.remove(INTERCEPT);
        let (Some((measure, scale)), Some(slope)) = (responder(&self.reg_responder), slope) else {
            variant.skip_measure();
            return;
        };
        if !self.reg_coeffs.is_empty() || responders.contains(&self.reg_responder) {
            variant.skip_measure();
            return;
        }
        responders.push(self.reg_responder);
        slope.insert(variant, &measure, "slope", scale);
    }
}

const ITERS: &str = "iters";
const INTERCEPT: &str = "y";

/// The measure of a responder criterion measures, and the scale of its slope per iteration
/// (criterion-measurement 0.2.3.0, `Criterion/Measurement/Types.hs:179-205`).
fn responder(responder: &str) -> Option<(MeasureNameId, Scale)> {
    Some(match responder {
        "time" => (Latency::name_id(), Scale::Seconds),
        "cpuTime" => (CpuTime::name_id(), Scale::Seconds),
        "cycles" => (Cycles::name_id(), Scale::AsWritten),
        "allocated" => (Allocated::name_id(), Scale::AsWritten),
        "bytesCopied" => (GcCopied::name_id(), Scale::AsWritten),
        "numGcs" => (GcCollections::name_id(), Scale::AsWritten),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
enum Scale {
    Seconds,
    AsWritten,
}

impl Scale {
    fn apply(self, value: Finite) -> Finite {
        match self {
            Self::Seconds => value.map(nanos),
            Self::AsWritten => value,
        }
    }
}

/// Seconds as nanoseconds, scaled in decimal from the shortest text that reads back as the
/// seconds, so that the nanoseconds hold the digits criterion wrote, read as the float nearest
/// them.
fn nanos(seconds: f64) -> f64 {
    seconds
        .to_string()
        .parse::<Decimal>()
        .ok()
        .and_then(|seconds| seconds.checked_mul(Decimal::from(NANOS_PER_SECOND)))
        .and_then(|nanos| nanos.to_string().parse::<f64>().ok())
        .unwrap_or(seconds * f64::from(NANOS_PER_SECOND))
}

const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// A bootstrap estimate in either shape criterion writes, where aeson writes `null` for NaN and a
/// string for an infinity (`Data/Aeson/Encoding/Internal.hs:340-342` at 2.3.2.0), or `null` for
/// either (`:296` at 1.3.1.1).
#[derive(Deserialize)]
#[serde(untagged)]
enum Estimate {
    Distances(DistancesEstimate),
    Bounds(BoundsEstimate),
}

impl Estimate {
    /// The estimate as `statistic`, and the bounds of its interval, named for the interval's
    /// level.
    fn insert(
        self,
        variant: &mut Variant<'_>,
        measure: &MeasureNameId,
        statistic: &str,
        scale: Scale,
    ) {
        let (point, lower, upper, level) = match self {
            Self::Distances(estimate) => estimate.bounds(),
            Self::Bounds(BoundsEstimate {
                point,
                lower,
                upper,
                level,
            }) => (point, lower, upper, level.get()),
        };
        let interval = level
            .and_then(percent)
            .map_or_else(|| "ci".to_owned(), |level| format!("ci{level}"));
        scale.apply(point).insert(variant, measure, statistic, 1.0);
        for (bound, side) in [(lower, "lower"), (upper, "upper")] {
            scale.apply(bound).insert_spread(
                point,
                variant,
                measure,
                &format!("{statistic}_{interval}_{side}"),
                1.0,
            );
        }
    }
}

/// statistics 0.14 and later: the interval as its distances from the estimate, and its level as
/// the significance (`Statistics/Types.hs:353-362` at 0.16.2.1).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistancesEstimate {
    est_point: Finite,
    est_error: ConfInt,
}

impl DistancesEstimate {
    /// The estimate, the bounds as statistics' `confidenceInterval` computes them
    /// (`Statistics/Types.hs:415-417` at 0.16.2.1), and the level.
    fn bounds(self) -> (Finite, Finite, Finite, Option<f64>) {
        let Self {
            est_point,
            est_error:
                ConfInt {
                    lower_delta,
                    upper_delta,
                    significance,
                },
        } = self;
        let point = est_point.get();
        let lower = point
            .zip(lower_delta.get())
            .map(|(point, delta)| point - delta);
        let upper = point
            .zip(upper_delta.get())
            .map(|(point, delta)| point + delta);
        (
            est_point,
            Finite::from(lower),
            Finite::from(upper),
            significance.get().map(|significance| 1.0 - significance),
        )
    }
}

#[derive(Deserialize)]
struct ConfInt {
    #[serde(rename = "confIntLDX")]
    lower_delta: Finite,
    #[serde(rename = "confIntUDX")]
    upper_delta: Finite,
    #[serde(rename = "confIntCL")]
    significance: Finite,
}

/// criterion 1.1.4.0 on statistics 0.13: the interval as its bounds, and its level as `-I` gave it
/// (`Statistics/Resampling/Bootstrap.hs:44-55` at 0.13.3.0, `Criterion/Main/Options.hs:107` at
/// 1.1.4.0).
#[derive(Deserialize)]
struct BoundsEstimate {
    #[serde(rename = "estPoint")]
    point: Finite,
    #[serde(rename = "estLowerBound")]
    lower: Finite,
    #[serde(rename = "estUpperBound")]
    upper: Finite,
    #[serde(rename = "estConfidenceLevel")]
    level: Finite,
}

/// The interval's level in percent as a metric name writes it, `95` or `99_9`. A later criterion
/// stores a level from `-I` as `1 - level` (`Criterion/Main/Options.hs:148` at 1.6.5.0), so the
/// level is rounded to 12 decimal places to read back as typed.
fn percent(level: f64) -> Option<String> {
    let percent = level
        .to_string()
        .parse::<Decimal>()
        .ok()?
        .round_dp(12)
        .checked_mul(Decimal::ONE_HUNDRED)?
        .normalize();
    Some(percent.to_string().replace('.', "_"))
}

#[cfg(test)]
#[expect(
    clippy::unreadable_literal,
    reason = "estimates as criterion wrote them, in nanoseconds"
)]
mod test_haskell_criterion {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{CpuTime, Cycles, Latency},
                dotnet::Allocated,
                haskell::{GcCollections, GcCopied},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_edited_v1, convert_v1, value,
        },
        results::adapter_results::Skipped,
    };

    use super::AdapterHaskellCriterion;

    /// A statistic with its interval at `level`, as `[estimate, lower, upper]`.
    fn estimate(
        statistic: &str,
        level: &str,
        [point, lower, upper]: [f64; 3],
    ) -> [(String, f64); 3] {
        [
            (statistic.to_owned(), point),
            (format!("{statistic}_ci{level}_lower"), lower),
            (format!("{statistic}_ci{level}_upper"), upper),
        ]
    }

    /// Latency's slope, mean, and standard deviation, each with its interval at `level`.
    fn latency(level: &str, [slope, mean, std_dev]: [[f64; 3]; 3]) -> Expected {
        (
            Latency::SLUG_STR,
            [
                estimate("slope", level, slope),
                estimate("mean", level, mean),
                estimate("std_dev", level, std_dev),
            ]
            .into_iter()
            .flatten()
            .map(|(name, value)| (&*name.leak(), value))
            .collect(),
        )
    }

    /// A responder's slope with its 95% interval.
    fn slope(measure: &'static str, slope: [f64; 3]) -> Expected {
        (
            measure,
            estimate("slope", "95", slope)
                .into_iter()
                .map(|(name, value)| (&*name.leak(), value))
                .collect(),
        )
    }

    /// Kills a leaf that drops an estimate or its interval, misnames one, swaps the lower and
    /// upper deltas or their signs, reads the intercept as the slope, leaves seconds unscaled,
    /// or records no leaf.
    #[test]
    fn adapter_haskell_criterion() {
        for adapter in [Adapter::Magic, Adapter::HaskellCriterion] {
            let results = convert_v1(adapter, "haskell/criterion/criterion.json");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::HaskellCriterion),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 5, "{adapter}");
            assert_variant(&results, "fib/10", "{}", &[latency("95", FIB_10)]);
            assert_variant(
                &results,
                "fib/with space",
                "{}",
                &[latency("95", FIB_WITH_SPACE)],
            );
            assert_eq!(results.skipped, Skipped::default(), "{adapter}");
        }
    }

    /// Kills a leaf that maps a responder to another measure, scales a count as seconds or CPU
    /// time not at all, reads a responder with no measure, or lets a repeated `time` regression
    /// overwrite the first, each without counting the skip.
    #[test]
    fn adapter_haskell_criterion_responders() {
        let results = convert_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion_rts.json",
        );
        assert_variant(
            &results,
            "fib/10",
            "{}",
            &[
                latency("95", RTS_FIB_10),
                slope(Allocated::SLUG_STR, RTS_ALLOCATED),
                slope(CpuTime::SLUG_STR, RTS_CPU_TIME),
            ],
        );
        assert_eq!(results.skipped.measures, 0);

        let results = convert_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion_responders.json",
        );
        assert_variant(
            &results,
            "fib/10",
            "{}",
            &[
                latency("95", RESPONDERS_FIB_10),
                slope(Cycles::SLUG_STR, RESPONDERS_CYCLES),
                slope(GcCollections::SLUG_STR, RESPONDERS_GC_COLLECTIONS),
                slope(GcCopied::SLUG_STR, RESPONDERS_GC_COPIED),
            ],
        );
        // `gcCpuSeconds`, and the repeated `time`.
        assert_eq!(results.skipped.measures, 2);
    }

    /// Kills a leaf that reads the `iters` coefficient of a regression with another predictor, or
    /// a regression on another predictor as the slope, or skips either without counting it.
    #[test]
    fn adapter_haskell_criterion_other_predictors() {
        let results = convert_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion_predictors.json",
        );
        assert_variant(
            &results,
            "fib/10",
            "{}",
            &[latency("95", PREDICTORS_FIB_10)],
        );
        // `allocated` on `iters` and `numGcs`, and `cpuTime` on `cycles`.
        assert_eq!(results.skipped.measures, 2);
    }

    /// Kills a leaf that names every interval 95%, names it by the significance, or drops a
    /// level's decimal.
    #[test]
    fn adapter_haskell_criterion_level() {
        for (fixture, level, expected) in [
            ("criterion_ci99.json", "99", CI99_FIB_10),
            ("criterion_ci99_9.json", "99_9", CI99_9_FIB_10),
        ] {
            let results = convert_v1(
                Adapter::HaskellCriterion,
                &format!("haskell/criterion/{fixture}"),
            );
            assert_variant(&results, "fib/10", "{}", &[latency(level, expected)]);
        }
    }

    /// `-I 0.1` and `-I 0.05` write `confIntCL` as 0.9 and 0.95, whose `1 - significance` is
    /// 0.09999999999999998 and 0.050000000000000044 as floats, so the real capture's level is
    /// replaced with each.
    ///
    /// Kills: a level put in percent without rounding, which names `ci9_999999999999998`.
    #[test]
    fn adapter_haskell_criterion_level_below_half() {
        for (significance, level) in [(0.9, "10"), (0.95, "5")] {
            let results = convert_edited_v1(
                Adapter::HaskellCriterion,
                "haskell/criterion/criterion.json",
                |report| {
                    report[2][0]["reportAnalysis"]["anMean"]["estError"]["confIntCL"] =
                        significance.into();
                },
            );
            let [_slope, mean, _std_dev] = FIB_10;
            for (metric, expected) in [
                (format!("mean_ci{level}_lower"), mean[1]),
                (format!("mean_ci{level}_upper"), mean[2]),
            ] {
                assert_eq!(
                    value(&results, "fib/10", "{}", Latency::SLUG_STR, &metric),
                    Some(expected),
                    "{metric}"
                );
            }
        }
    }

    /// statistics writes a level that is not finite as `null`, which `-I` never gives
    /// (`Criterion/Main/Options.hs:148`), so the real capture's level is replaced with it; the leaf
    /// then names the interval with no level, as it assumes none.
    ///
    /// Kills: a leaf that drops the interval, or names a level it cannot read.
    #[test]
    fn adapter_haskell_criterion_no_level() {
        let results = convert_edited_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion.json",
            |report| {
                report[2][0]["reportAnalysis"]["anMean"]["estError"]["confIntCL"] =
                    serde_json::Value::Null;
            },
        );
        let [_slope, mean, _std_dev] = FIB_10;
        for (metric, expected) in [
            ("mean", mean[0]),
            ("mean_ci_lower", mean[1]),
            ("mean_ci_upper", mean[2]),
        ] {
            assert_eq!(
                value(&results, "fib/10", "{}", Latency::SLUG_STR, metric),
                Some(expected),
                "{metric}"
            );
        }
        assert_eq!(
            value(
                &results,
                "fib/10",
                "{}",
                Latency::SLUG_STR,
                "mean_ci95_lower"
            ),
            None
        );
    }

    /// Kills a leaf that lets a repeated name overwrite the first benchmark, or skips the repeat
    /// or an empty name without counting it.
    #[test]
    fn adapter_haskell_criterion_duplicate() {
        let results = convert_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion_duplicate.json",
        );
        assert_eq!(results.inner.len(), 1);
        assert_eq!(
            value(&results, "fib/10", "{}", Latency::SLUG_STR, "mean"),
            Some(52.29117676441254)
        );
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 1);
    }

    /// The real capture keeps only its third report, the one with an empty name.
    ///
    /// Kills: a leaf that refuses a report whose every benchmark it skipped, so the count never
    /// reaches the report.
    #[test]
    fn adapter_haskell_criterion_only_skipped() {
        let results = convert_edited_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion_duplicate.json",
            |report| {
                let reports = report[2].as_array_mut().expect("the reports");
                reports.drain(..2);
            },
        );
        assert!(results.inner.is_empty());
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// criterion 1.1.4.0 on statistics 0.13 writes each interval as its bounds and its level.
    ///
    /// Kills: a leaf that refuses that shape, reads its bounds as distances from the estimate, or
    /// reads its level as a significance.
    #[test]
    fn adapter_haskell_criterion_1_1_4_0() {
        for adapter in [Adapter::Magic, Adapter::HaskellCriterion] {
            let results = convert_v1(
                adapter,
                "haskell/criterion/bench_1_0_9_criterion_1_1_4_0.json",
            );
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::HaskellCriterion),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 2, "{adapter}");
            assert_variant(
                &results,
                "bench/true",
                "{}",
                &[latency("95", BENCH_1_0_9_CRITERION_1_1_4_0_TRUE)],
            );
            assert_variant(
                &results,
                "bench/sleep 0.01",
                "{}",
                &[latency("95", BENCH_1_0_9_CRITERION_1_1_4_0_SLEEP)],
            );
            assert_eq!(results.skipped, Skipped::default(), "{adapter}");
        }
    }

    /// `bench` names a single command bare and groups several under `bench`, and its last GitHub
    /// release runs on criterion 1.4.1.0 or 1.2.6.0, whose header is `criterio`.
    ///
    /// Kills: a leaf that names `bench`'s commands itself, or refuses an older criterion or its
    /// header.
    #[test]
    fn adapter_haskell_criterion_bench() {
        for (fixture, benchmarks, name, expected) in [
            (
                "bench.json",
                ["bench/true", "bench/sleep 0.01"].as_slice(),
                "bench/sleep 0.01",
                BENCH_SLEEP,
            ),
            (
                "bench_single.json",
                ["true"].as_slice(),
                "true",
                BENCH_SINGLE_TRUE,
            ),
            (
                "bench_1_0_9.json",
                ["bench/true", "bench/sleep 0.01"].as_slice(),
                "bench/true",
                BENCH_1_0_9_TRUE,
            ),
            (
                "bench_1_0_9_criterion_1_2_6_0.json",
                ["bench/true", "bench/sleep 0.01"].as_slice(),
                "bench/sleep 0.01",
                BENCH_1_0_9_CRITERION_1_2_6_0_SLEEP,
            ),
        ] {
            let results = convert_v1(Adapter::Magic, &format!("haskell/criterion/{fixture}"));
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::HaskellCriterion),
                "{fixture}"
            );
            let mut names: Vec<String> = results.inner.keys().map(ToString::to_string).collect();
            names.sort();
            let mut expected_names: Vec<&str> = benchmarks.to_vec();
            expected_names.sort_unstable();
            assert_eq!(names, expected_names, "{fixture}");
            assert_variant(&results, name, "{}", &[latency("95", expected)]);
        }
    }

    /// criterion writes `fib/20`'s slope as `1.228873343770806e-7` seconds, which is exactly
    /// 122.8873343770806 ns, where a float product gives 122.88733437708059.
    ///
    /// Kills: seconds scaled to nanoseconds in floating point, or through a decimal's nearest
    /// float conversion instead of its digits.
    #[test]
    fn adapter_haskell_criterion_nanoseconds_in_decimal() {
        let results = convert_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion.json",
        );
        let slope = value(&results, "fib/20", "{}", Latency::SLUG_STR, "slope").map(f64::to_bits);
        assert_eq!(slope, Some(122.8873343770806f64.to_bits()));
    }

    /// aeson writes NaN as `null` and an infinity as a string (`Data/Aeson/Encoding/Internal.hs:340-342`
    /// at 2.3.2.0); no capture holds one, so the real capture's bounds and estimates are replaced
    /// with what aeson writes for them.
    ///
    /// Kills: a leaf that refuses the report for a value that is not finite, reads one as a
    /// number, or counts a bound that is not finite beside a finite estimate.
    #[test]
    fn adapter_haskell_criterion_not_finite() {
        let results = convert_edited_v1(
            Adapter::HaskellCriterion,
            "haskell/criterion/criterion.json",
            |report| {
                let analysis = &mut report[2][0]["reportAnalysis"];
                analysis["anStdDev"]["estError"]["confIntLDX"] = serde_json::Value::Null;
                analysis["anRegress"][0]["regCoeffs"]["iters"]["estError"]["confIntUDX"] =
                    "+inf".into();
                analysis["anMean"]["estPoint"] = serde_json::Value::Null;
            },
        );
        let [slope, _mean, std_dev] = FIB_10;
        assert_variant(
            &results,
            "fib/10",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("slope", slope[0]),
                    ("slope_ci95_lower", slope[1]),
                    ("std_dev", std_dev[0]),
                    ("std_dev_ci95_upper", std_dev[2]),
                ],
            )],
        );
        // The mean, and its 2 bounds, which a mean that is not finite leaves with no value.
        assert_eq!(results.skipped.metrics, 3);
    }

    /// Kills a leaf that reads the first of several reports and drops the rest.
    #[test]
    fn adapter_haskell_criterion_refuses_several_reports() {
        let input = ["criterion.json", "criterion_rts.json"]
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/haskell/criterion/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n");
        assert_eq!(
            AdapterHaskellCriterion::parse(&input, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_haskell_criterion_ignores_average() {
        for suffix in [
            "haskell/criterion/criterion.json",
            "haskell/criterion/criterion_rts.json",
        ] {
            assert_ignores_average(Adapter::HaskellCriterion, suffix);
        }
    }

    const FIB_10: [[f64; 3]; 3] = [
        [56.81102030978158, 54.57511107930058, 59.43617930157705],
        [58.50628657108838, 56.43599290315742, 59.74910058530811],
        [4.069011323941324, 2.8338353283018725, 5.73181306686779],
    ];
    const FIB_WITH_SPACE: [[f64; 3]; 3] = [
        [186.23626346874514, 181.10451522357786, 192.5389275785466],
        [192.92127456971272, 186.7686549348119, 199.59775397168224],
        [16.5655167555902, 14.117982270029238, 20.120065267828686],
    ];
    const RTS_FIB_10: [[f64; 3]; 3] = [
        [53.93257883856353, 52.93714206555653, 54.96827022148636],
        [54.86176028166701, 53.56786266978812, 56.23386335355574],
        [2.907439395470838, 2.098972500749719, 3.8019515804363424],
    ];
    const RTS_ALLOCATED: [f64; 3] = [480.00005690092155, 479.9992039880652, 480.00101375087615];
    const RTS_CPU_TIME: [f64; 3] = [53.99296837744352, 53.09116605297978, 55.03418992234432];
    const RESPONDERS_FIB_10: [[f64; 3]; 3] = [
        [60.44717063681524, 60.00295360788791, 60.90767957912435],
        [60.46080739860406, 59.99375444311853, 60.82984311719123],
        [0.9188293178993377, 0.6176828440839444, 1.3188290664992324],
    ];
    const RESPONDERS_CYCLES: [f64; 3] =
        [253.48695255733605, 251.68207551073345, 255.55943258032502];
    const RESPONDERS_GC_COLLECTIONS: [f64; 3] = [
        0.0001139821388178504,
        0.00011371167851711844,
        0.00011418996093943032,
    ];
    const RESPONDERS_GC_COPIED: [f64; 3] = [
        0.02022792248721615,
        0.02003613282163964,
        0.02055010336977823,
    ];
    const PREDICTORS_FIB_10: [[f64; 3]; 3] = [
        [60.34307513249244, 59.44278876276535, 61.42187487779402],
        [60.81917625398654, 59.90604908169494, 63.2351446825635],
        [2.9945683520017483, 1.1117800274279468, 5.582202689709726],
    ];
    const CI99_FIB_10: [[f64; 3]; 3] = [
        [58.96220226013539, 57.87686579113081, 59.95801790430059],
        [59.026697805983574, 57.98538369975066, 59.92673284251929],
        [1.6769586946379467, 1.1050889693442842, 2.412237419144749],
    ];
    const CI99_9_FIB_10: [[f64; 3]; 3] = [
        [58.53763314061307, 54.44287729271926, 68.81601601473987],
        [59.22650236536743, 55.12202288260713, 66.93498590539765],
        [8.812690713843955, 3.317803727380947, 14.766195750773928],
    ];
    const BENCH_SLEEP: [[f64; 3]; 3] = [
        [10976134.640149323, 10911074.751219856, 11051393.728883497],
        [10913063.533062767, 10868308.681773243, 10954185.750356838],
        [74872.5664634339, 60128.78802309235, 91209.74013314789],
    ];
    const BENCH_SINGLE_TRUE: [[f64; 3]; 3] = [
        [294740.8066820205, 284788.60171241214, 306375.0489379155],
        [298085.6422976435, 289584.4251884895, 307765.94327331806],
        [18703.67149572479, 15922.333747260443, 22734.870453684132],
    ];
    const BENCH_1_0_9_CRITERION_1_2_6_0_SLEEP: [[f64; 3]; 3] = [
        [11110278.034511095, 10967395.04168097, 11206786.061566757],
        [11075423.05494105, 11016109.75216368, 11128367.509808375],
        [96372.36867484683, 68583.12344984195, 140269.71157256825],
    ];
    const BENCH_1_0_9_CRITERION_1_1_4_0_TRUE: [[f64; 3]; 3] = [
        [321315.934119005, 309517.6778154995, 345110.5728847174],
        [361597.3486103056, 330089.027207487, 494801.3634995598],
        [123518.4820025038, 28457.49518483684, 248051.0888305008],
    ];
    const BENCH_1_0_9_CRITERION_1_1_4_0_SLEEP: [[f64; 3]; 3] = [
        [11312894.30863022, 10995770.638873523, 11626293.117623717],
        [11279981.185067328, 11187393.291090203, 11398709.58621423],
        [175513.12520121533, 107253.58102208491, 255550.4842810821],
    ];
    const BENCH_1_0_9_TRUE: [[f64; 3]; 3] = [
        [297384.4517281602, 291239.7996559753, 303908.77231732954],
        [299472.9730236818, 294273.2394222172, 305699.8946422372],
        [11709.89458788539, 9468.705505563355, 15012.442798812617],
    ];
}
