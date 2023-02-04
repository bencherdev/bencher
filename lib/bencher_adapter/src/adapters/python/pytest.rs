use bencher_json::{BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterPythonPytest;

impl Adapter for AdapterPythonPytest {
    fn parse(input: &str) -> Option<AdapterResults> {
        serde_json::from_str::<Pytest>(input)
            .ok()?
            .try_into()
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
    pub median: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub iqr: Decimal,
}

impl TryFrom<Pytest> for Option<AdapterResults> {
    type Error = AdapterError;

    fn try_from(dot_net: Pytest) -> Result<Self, Self::Error> {
        let benchmarks = dot_net.benchmarks.0;
        let mut benchmark_metrics = Vec::with_capacity(benchmarks.len());
        for benchmark in benchmarks {
            let Benchmark {
                fullname: benchmark_name,
                stats,
            } = benchmark;
            let Stats { median, iqr } = stats;

            // JSON output is always in seconds
            let units = Units::Sec;
            let value = latency_as_nanos(median, units);
            let range = latency_as_nanos(iqr, units);
            let json_metric = JsonMetric {
                value,
                lower_bound: Some(value - range),
                upper_bound: Some(value + range),
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
        adapters::test_util::{convert_file_path, validate_latency},
        AdapterResults,
    };

    use super::AdapterPythonPytest;

    fn convert_python_pytest(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/python/pytest/{suffix}.json");
        convert_file_path::<AdapterPythonPytest>(&file_path)
    }

    #[test]
    fn test_adapter_python_pytest_two() {
        let results = convert_python_pytest("two");
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
