use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, PeakMemory},
        },
        report::Adapter,
    },
};
use serde::{Deserialize, Deserializer, de::Error as _};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{finite::Finite, leaf::LeafResults},
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// pyperf's JSON file, in format `1.0`, 6, or 5, each benchmark's statistics computed from the
/// values of all its runs as pyperf computes them, citing pyperf 2.11.0's `pyperf/` sources.
pub struct AdapterPythonPyperf;

impl Adaptable for AdapterPythonPyperf {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Suite {
            version,
            metadata,
            benchmarks,
        } = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        for benchmark in &benchmarks {
            benchmark.insert(version, metadata.as_ref(), &mut results);
        }
        results.finish_with_skips(Adapter::PythonPyperf)
    }
}

#[derive(Deserialize)]
struct Suite {
    version: Format,
    metadata: Option<Metadata>,
    benchmarks: Vec<Benchmark>,
}

/// The JSON format, by how it is read: format 6 renamed a run's `samples` to `values` and a
/// benchmark's `common_metadata` to `metadata`, and `1.0` changed only the warmups
/// (`_bench.py:16-31`, `:260-276`, `:542-546`).
#[derive(Clone, Copy)]
enum Format {
    Samples,
    Values,
}

impl<'de> Deserialize<'de> for Format {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let version = serde_json::Value::deserialize(deserializer)?;
        if version == "1.0" || version == 6 {
            Ok(Self::Values)
        } else if version == 5 {
            Ok(Self::Samples)
        } else {
            Err(D::Error::custom(format!(
                "expected pyperf format 1.0, 6, or 5, found {version}"
            )))
        }
    }
}

#[derive(Deserialize)]
struct Benchmark {
    metadata: Option<Metadata>,
    common_metadata: Option<Metadata>,
    runs: Vec<Run>,
}

impl Benchmark {
    /// The benchmark under its name and unit, which pyperf reads from its first run's metadata,
    /// itself the benchmark's over the suite's (`_bench.py:260-263`, `:342-344`, `:498-500`,
    /// `:547-549`), or counted as skipped when it has no value or no measure.
    fn insert(&self, format: Format, suite: Option<&Metadata>, results: &mut LeafResults) {
        let metadata = match format {
            Format::Samples => self.common_metadata.as_ref(),
            Format::Values => self.metadata.as_ref(),
        };
        let name = metadata
            .and_then(|metadata| metadata.name.as_deref())
            .or_else(|| suite.and_then(|suite| suite.name.as_deref()));
        let unit = metadata
            .and_then(|metadata| metadata.unit.as_deref())
            .or_else(|| suite.and_then(|suite| suite.unit.as_deref()));
        let values = self
            .runs
            .iter()
            .flat_map(|run| run.values(format))
            .copied()
            .collect();
        let (Some(name), Some((measure, factor)), Some(statistics)) = (
            name.and_then(|name| name.parse::<BenchmarkName>().ok()),
            measure_of(unit),
            statistics(values),
        ) else {
            results.skip_benchmark();
            return;
        };
        if let Some(mut variant) = results.new_variant(name, ParameterSet::default()) {
            for (metric, value) in statistics {
                Finite::from(Some(value)).insert(&mut variant, &measure, metric, factor);
            }
        }
    }
}

/// The metadata the leaf reads; the rest, such as each run's peak memory, stays unread.
#[derive(Deserialize)]
struct Metadata {
    name: Option<String>,
    unit: Option<String>,
}

#[derive(Deserialize)]
struct Run {
    values: Option<Vec<f64>>,
    samples: Option<Vec<f64>>,
}

impl Run {
    /// The run's values, each the time or size of one loop, and none for a calibration run.
    fn values(&self, format: Format) -> &[f64] {
        match format {
            Format::Samples => self.samples.as_deref(),
            Format::Values => self.values.as_deref(),
        }
        .unwrap_or_default()
    }
}

/// The measure of pyperf's `unit`, seconds when none is set (`_bench.py:498-500`), and the
/// factor to its unit, or `None` for `integer`, which names no quantity.
fn measure_of(unit: Option<&str>) -> Option<(MeasureNameId, f64)> {
    match unit {
        None | Some("second") => Some((Latency::name_id(), Units::Sec.as_nanos())),
        Some("byte") => Some((PeakMemory::name_id(), 1.0)),
        Some(_) => None,
    }
}

/// What `pyperf stats` prints from a benchmark's values (`_bench.py:394-425`, `:468-471`,
/// `_cli.py:274-329`), with the standard deviation from two values, as pyperf's result line
/// prints it (`_cli.py:499-505`), or `None` when there is no value.
fn statistics(mut values: Vec<f64>) -> Option<Vec<(&'static str, f64)>> {
    values.sort_by(f64::total_cmp);
    let (min, max) = (*values.first()?, *values.last()?);
    let count = f64::from(u32::try_from(values.len()).ok()?);
    let mean = values.iter().sum::<f64>() / count;
    let median = median_of(&values)?;
    let (p25, p75) = (percentile(&values, 25.0)?, percentile(&values, 75.0)?);
    let iqr = p75 - p25;
    let mut statistics = vec![
        ("mean", mean),
        ("median", median),
        ("min", min),
        ("max", max),
        ("p5", percentile(&values, 5.0)?),
        ("p25", p25),
        ("p75", p75),
        ("p95", percentile(&values, 95.0)?),
        ("lower_fence", p25 - 1.5 * iqr),
        ("upper_fence", p75 + 1.5 * iqr),
    ];
    if values.len() > 1 {
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (count - 1.0);
        statistics.push(("std_dev", variance.sqrt()));
    }
    if values.len() > 2 {
        let mut deviations: Vec<f64> = values.iter().map(|value| (median - value).abs()).collect();
        deviations.sort_by(f64::total_cmp);
        statistics.push(("median_abs_dev", median_of(&deviations)?));
    }
    Some(statistics)
}

/// The median of sorted values, the mean of the middle two for an even count, as Python's
/// `statistics.median` computes it.
fn median_of(sorted: &[f64]) -> Option<f64> {
    let middle = sorted.len().div_euclid(2);
    let upper = *sorted.get(middle)?;
    if sorted.len().is_multiple_of(2) {
        Some(f64::midpoint(*sorted.get(middle.checked_sub(1)?)?, upper))
    } else {
        Some(upper)
    }
}

/// pyperf's percentile of sorted values, linear between the two closest ranks
/// (`_utils.py:389-405`).
fn percentile(sorted: &[f64], percent: f64) -> Option<f64> {
    let last = u32::try_from(sorted.len().checked_sub(1)?).ok()?;
    let rank = f64::from(last) * (percent / 100.0);
    let (floor, ceil) = (rank.floor(), rank.ceil());
    let index = (1..=last)
        .take_while(|index| f64::from(*index) <= floor)
        .count();
    let lower = *sorted.get(index)?;
    if ceil > floor {
        Some(lower * (ceil - rank) + *sorted.get(index + 1)? * (rank - floor))
    } else {
        Some(lower)
    }
}

#[cfg(test)]
mod test_python_pyperf {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, PeakMemory},
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{
        Expected, assert_ignores_average, assert_value, assert_variant, convert_v1, variants,
    };

    /// Kills a statistic misnamed or left in seconds, the deviation of the population for the
    /// sample's, the median absolute deviation about the mean, the upper of the middle two values
    /// for the median of an even count, a percentile at the nearest rank rather than between the
    /// two closest, a fence from the wrong quartile or at another multiple of the interquartile
    /// range, a warmup read as a value, the runs' values not pooled, and another leaf recorded.
    #[test]
    fn adapter_python_pyperf_suite() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/suite_fast.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::PythonPyperf));
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 3);
        assert_variant(
            &results,
            "fib_10",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 133.828_194_475_181_28),
                    ("median", 132.359_858_512_852_7),
                    ("min", 129.141_598_701_485_24),
                    ("max", 141.528_637_886_121_66),
                    ("p5", 129.194_299_268_626_82),
                    ("p25", 129.883_329_153_093_8),
                    ("p75", 136.361_407_041_527_85),
                    ("p95", 141.470_455_169_740_32),
                    ("lower_fence", 120.166_212_320_442_7),
                    ("upper_fence", 146.078_523_874_178_96),
                    ("std_dev", 4.436_629_294_219_786),
                    ("median_abs_dev", 2.972_913_265_282_366),
                ],
            )],
        );
        for (name, median) in [
            ("fib 20", 294.861_970_901_523_65),
            ("sort_list", 2_523.414_260_865_129_6),
        ] {
            assert_value(
                &results,
                (name, "{}"),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a name read only from the benchmark's own metadata, where pyperf hoists it to the
    /// file's when the file holds one benchmark, and a run's metadata read as a measure:
    /// `pyperf command` records each run's peak memory there.
    #[test]
    fn adapter_python_pyperf_command() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/command.json");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "command",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 7_661_931.115_624_298),
                    ("median", 7_762_759.468_750_602),
                    ("min", 6_426_588.562_497_671),
                    ("max", 8_692_387.249_993_771),
                    ("p5", 6_705_610.212_495_116_5),
                    ("p25", 7_471_901.031_250_283),
                    ("p75", 8_062_168.515_625_245),
                    ("p95", 8_337_634.924_999_547),
                    ("lower_fence", 6_586_499.804_687_84),
                    ("upper_fence", 8_947_569.742_187_688),
                    ("std_dev", 599_179.804_994_994_9),
                    ("median_abs_dev", 301_124.749_999_104_3),
                ],
            )],
        );
    }

    /// Kills nanobench's pyperf template refused or left unnamed: it has no metadata on its
    /// benchmark or its run, and its name and unit are the whole file's.
    #[test]
    fn adapter_python_pyperf_nanobench_template() {
        let results = convert_v1(Adapter::PythonPyperf, "cpp/nanobench/pyperf_template.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::PythonPyperf));
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "Strings",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 11.170_746_851_014_464),
                    ("median", 11.207_387_154_027),
                    ("min", 10.825_073_331_370_6),
                    ("max", 11.584_633_670_276),
                    ("p5", 10.837_587_227_019_3),
                    ("p25", 11.031_245_145_451_3),
                    ("p75", 11.306_788_068_994_45),
                    ("p95", 11.456_716_285_128_051),
                    ("lower_fence", 10.617_930_760_136_575),
                    ("upper_fence", 11.720_102_454_309_176),
                    ("std_dev", 0.228_463_009_069_160_87),
                    ("median_abs_dev", 0.120_520_545_868_998_9),
                ],
            )],
        );
    }

    /// Kills format 6 refused, its runs read by format 5's `samples` or its benchmarks by
    /// format 5's `common_metadata`, its file's metadata left unread, and a benchmark with no
    /// unit anywhere, which pyperf times in seconds, skipped.
    #[test]
    fn adapter_python_pyperf_format_6() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/format6.json");
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 3);
        for (name, mean) in [
            ("fib_10", 148.859_489_632_058_3),
            ("fib 20", 297.425_627_424_075),
            ("sort_list", 2_706.564_844_445_758),
        ] {
            assert_value(&results, (name, "{}"), (Latency::SLUG_STR, "mean"), mean);
        }
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/timeit6.json");
        assert!(results.skipped.is_empty());
        assert_value(
            &results,
            ("timeit", "{}"),
            (Latency::SLUG_STR, "mean"),
            269.598_915_297_875,
        );
    }

    /// Kills format 5 refused, its runs read by format 6's `values` rather than their `samples`,
    /// and its benchmarks named by format 6's `metadata` rather than their `common_metadata`.
    #[test]
    fn adapter_python_pyperf_format_5() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/format5.json");
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 3);
        for (name, mean) in [
            ("fib_10", 137.473_420_000_477_92),
            ("fib 20", 261.705_396_839_562_2),
            ("sort_list", 2_328.676_329_788_06),
        ] {
            assert_value(&results, (name, "{}"), (Latency::SLUG_STR, "mean"), mean);
        }
    }

    /// Kills the byte unit put in latency, scaled as seconds, or left unread from the file's
    /// metadata.
    #[test]
    fn adapter_python_pyperf_memory() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/track_memory.json");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "fib_10",
            "{}",
            &[(
                PeakMemory::SLUG_STR,
                vec![
                    ("mean", 12_627_148.8),
                    ("median", 12_578_816.0),
                    ("min", 12_562_432.0),
                    ("max", 12_726_272.0),
                    ("p5", 12_564_275.200_000_001),
                    ("p25", 12_571_648.0),
                    ("p75", 12_699_648.0),
                    ("p95", 12_722_585.600_000_001),
                    ("lower_fence", 12_379_648.0),
                    ("upper_fence", 12_891_648.0),
                    ("std_dev", 71_905.251_158_575_18),
                    ("median_abs_dev", 14_336.0),
                ],
            )],
        );
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/tracemalloc.json");
        assert!(results.skipped.is_empty());
        assert_value(
            &results,
            ("fib_10", "{}"),
            (PeakMemory::SLUG_STR, "median"),
            210_815.0,
        );
    }

    /// Kills the `integer` unit stored under a measure it does not name, read as seconds when
    /// it is the file's, or skipped uncounted.
    #[test]
    fn adapter_python_pyperf_integer() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/loops.json");
        assert!(results.inner.is_empty());
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a benchmark with only a calibration run, which holds no value, skipped uncounted.
    #[test]
    fn adapter_python_pyperf_calibration_only() {
        let results = convert_v1(Adapter::PythonPyperf, "python/pyperf/calibration.json");
        assert!(results.inner.is_empty());
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a standard deviation computed from one value, or withheld from two where pyperf's
    /// result line prints it, and a median absolute deviation below the three values
    /// `pyperf stats` needs to print one, or withheld from three.
    #[test]
    fn adapter_python_pyperf_few_values() {
        let one = [
            "mean",
            "median",
            "min",
            "max",
            "p5",
            "p25",
            "p75",
            "p95",
            "lower_fence",
            "upper_fence",
        ]
        .map(|name| (name, 301.784_492_395_817_95));
        assert_few("one_value", &[(Latency::SLUG_STR, one.to_vec())]);
        assert_few(
            "two_values",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 333.171_134_986_320_9),
                    ("median", 333.171_134_986_320_9),
                    ("min", 322.178_516_420_912_33),
                    ("max", 344.163_753_551_729_4),
                    ("p5", 323.277_778_277_453_2),
                    ("p25", 327.674_825_703_616_6),
                    ("p75", 338.667_444_269_025_17),
                    ("p95", 343.064_491_695_188_6),
                    ("lower_fence", 311.185_897_855_503_75),
                    ("upper_fence", 355.156_372_117_138),
                    ("std_dev", 15.545_910_261_195_067),
                ],
            )],
        );
        assert_few(
            "three_values",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 306.860_725_403_831),
                    ("median", 311.721_176_149_415_44),
                    ("min", 269.138_206_498_986_7),
                    ("max", 339.722_793_563_090_9),
                    ("p5", 273.396_503_464_029_6),
                    ("p25", 290.429_691_324_201_03),
                    ("p75", 325.721_984_856_253_13),
                    ("p95", 336.922_631_821_723_3),
                    ("lower_fence", 237.491_251_026_122_88),
                    ("upper_fence", 378.660_425_154_331_3),
                    ("std_dev", 35.542_424_915_025_36),
                    ("median_abs_dev", 28.001_617_413_675_46),
                ],
            )],
        );
    }

    fn assert_few(fixture: &str, expected: &[Expected]) {
        let results = convert_v1(
            Adapter::PythonPyperf,
            &format!("python/pyperf/{fixture}.json"),
        );
        assert!(results.skipped.is_empty(), "{fixture}");
        assert_variant(&results, "timeit", "{}", expected);
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_python_pyperf_ignores_average() {
        for suffix in [
            "python/pyperf/suite_fast.json",
            "python/pyperf/format5.json",
        ] {
            assert_ignores_average(Adapter::PythonPyperf, suffix);
        }
    }
}
