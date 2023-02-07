use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError, Settings,
};

pub struct AdapterPythonPytest;

impl Adapter for AdapterPythonPytest {
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
            let bound = latency_as_nanos(spread, units);
            let json_metric = JsonMetric {
                value,
                lower_bound: Some(value - bound),
                upper_bound: Some(value + bound),
            };

            benchmark_metrics.push((benchmark_name, json_metric));
        }

        Ok(AdapterResults::new_latency(benchmark_metrics))
    }
}

#[cfg(test)]
pub(crate) mod test_python_pytest {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, convert_file_path_median, validate_latency},
        AdapterResults,
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
            24088.681333229408,
            Some(17913.59114336808),
            Some(30263.771523090734),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            2985030.672661863,
            Some(2810500.507247766),
            Some(3159560.8380759596),
        );
    }

    #[test]
    fn test_adapter_python_pytest_two_median() {
        let results = convert_python_pytest_median("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            22300.000000363696,
            Some(21033.00000000363),
            Some(23567.00000072376),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            2960582.5000003083,
            Some(2740893.5000006184),
            Some(3180271.499999998),
        );
    }

    #[test]
    fn test_adapter_python_pytest_four() {
        let results = convert_python_pytest("four");
        validate_adapter_python_pytest(results);
    }

    pub fn validate_adapter_python_pytest(results: AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("bench.py::test_fib_1").unwrap();
        validate_latency(
            metrics,
            149.95610248628836,
            Some(120.60437053414898),
            Some(179.30783443842773),
        );

        let metrics = results.get("bench.py::test_sleep_2").unwrap();
        validate_latency(
            metrics,
            2003843046.9999998,
            Some(2001965388.274841),
            Some(2005720705.7251585),
        );

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            28857.54012484424,
            Some(23621.602642835765),
            Some(34093.47760685271),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            3611916.368852473,
            Some(3238118.0866346513),
            Some(3985714.6510702944),
        );
    }

    #[test]
    fn test_adapter_python_pytest_four_median() {
        let results = convert_python_pytest_median("four");
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("bench.py::test_fib_1").unwrap();
        validate_latency(
            metrics,
            143.7600000020467,
            Some(143.09000000434224),
            Some(144.42999999975115),
        );

        let metrics = results.get("bench.py::test_sleep_2").unwrap();
        validate_latency(
            metrics,
            2005124842.999999,
            Some(2002304321.9999988),
            Some(2007945363.9999993),
        );

        let metrics = results.get("bench.py::test_fib_10").unwrap();
        validate_latency(
            metrics,
            28052.999999861328,
            Some(27927.999999732834),
            Some(28177.99999998982),
        );

        let metrics = results.get("bench.py::test_fib_20").unwrap();
        validate_latency(
            metrics,
            3471104.000000169,
            Some(3369463.000000072),
            Some(3572745.000000266),
        );
    }
}
