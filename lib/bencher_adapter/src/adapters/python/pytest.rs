use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adaptable, AdapterError, Settings,
};

pub struct AdapterPythonPytest;

impl Adaptable for AdapterPythonPytest {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str::<Pytest>(input)
            .ok()?
            .convert(settings)
            .ok()?
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Pytest {
    pub machine_info: JsonEmpty,
    pub commit_info: JsonEmpty,
    pub benchmarks: Benchmarks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Benchmarks(pub Vec<Benchmark>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Benchmark {
    pub fullname: BenchmarkName,
    pub stats: Stats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Stats {
    #[serde(with = "rust_decimal::serde::float")]
    pub mean: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub stddev: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub median: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub iqr: Decimal,
}

impl Pytest {
    #[allow(clippy::unnecessary_wraps)]
    fn convert(self, settings: Settings) -> Result<Option<AdapterResults>, AdapterError> {
        let benchmarks = self.benchmarks.0;
        let mut benchmark_metrics = Vec::with_capacity(benchmarks.len());
        for benchmark in benchmarks {
            let Benchmark {
                fullname: benchmark_name,
                stats,
            } = benchmark;
            let Stats {
                mean,
                stddev,
                median,
                iqr,
            } = stats;

            // JSON output is always in seconds
            let units = Units::Sec;
            let (average, spread) = match settings.average.unwrap_or_default() {
                JsonAverage::Mean => (mean, stddev),
                JsonAverage::Median => (median, iqr),
            };
            let value = latency_as_nanos(average, units);
            let spread = latency_as_nanos(spread, units);
            let json_metric = JsonMetric {
                value,
                lower_value: Some(value - spread),
                upper_value: Some(value + spread),
            };

            benchmark_metrics.push((benchmark_name, json_metric));
        }

        Ok(AdapterResults::new_latency(benchmark_metrics))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_python_pytest {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{
            convert_file_path, convert_file_path_median, opt_convert_file_path, validate_latency,
        },
        AdapterResults, Settings,
    };

    use super::AdapterPythonPytest;

    fn convert_python_pytest(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path::<AdapterPythonPytest>(&file_path)
    }

    fn convert_python_pytest_median(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path_median::<AdapterPythonPytest>(&file_path)
    }

    fn file_path(suffix: &str) -> String {
        format!("./tool_output/python/pytest/{suffix}.json")
    }

    #[test]
    fn test_adapter_python_pytest_two() {
        let results = convert_python_pytest("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            24_088.681_333_229_408,
            Some(17_913.591_143_368_08),
            Some(30_263.771_523_090_734),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            2_985_030.672_661_863,
            Some(2_810_500.507_247_766),
            Some(3_159_560.838_075_959_6),
        );
    }

    #[test]
    fn test_adapter_python_pytest_two_median() {
        let results = convert_python_pytest_median("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            22_300.000_000_363_696,
            Some(21_033.000_000_003_63),
            Some(23_567.000_000_723_76),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            2_960_582.500_000_308_3,
            Some(2_740_893.500_000_618_4),
            Some(3_180_271.499_999_998),
        );
    }

    #[test]
    fn test_adapter_python_pytest_four() {
        let four = "four";
        let file_path = file_path(four);

        let results = convert_python_pytest(four);
        validate_adapter_python_pytest(&results);

        let results = opt_convert_file_path::<AdapterPythonPytest>(
            &file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_python_pytest(&results);

        let results = convert_python_pytest_median(four);
        validate_adapter_python_pytest_median(&results);
    }

    pub fn validate_adapter_python_pytest(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("bench.py::test_fib_1").unwrap();
        validate_latency(
            metrics,
            149.956_102_486_288_36,
            Some(120.604_370_534_148_98),
            Some(179.307_834_438_427_73),
        );

        let metrics = results.get("bench.py::test_sleep_2").unwrap();
        validate_latency(
            metrics,
            2_003_843_046.999_999_8,
            Some(2_001_965_388.274_841),
            Some(2_005_720_705.725_158_5),
        );

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            28_857.540_124_844_24,
            Some(23_621.602_642_835_765),
            Some(34_093.477_606_852_71),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            3_611_916.368_852_473,
            Some(3_238_118.086_634_651_3),
            Some(3_985_714.651_070_294_4),
        );
    }

    fn validate_adapter_python_pytest_median(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("bench.py::test_fib_1").unwrap();
        validate_latency(
            metrics,
            143.760_000_002_046_7,
            Some(143.090_000_004_342_24),
            Some(144.429_999_999_751_15),
        );

        let metrics = results.get("bench.py::test_sleep_2").unwrap();
        validate_latency(
            metrics,
            2_005_124_842.999_999,
            Some(2_002_304_321.999_998_8),
            Some(2_007_945_363.999_999_3),
        );

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            28_052.999_999_861_328,
            Some(27_927.999_999_732_834),
            Some(28_177.999_999_989_82),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            3_471_104.000_000_169,
            Some(3_369_463.000_000_072),
            Some(3_572_745.000_000_266),
        );
    }
}
