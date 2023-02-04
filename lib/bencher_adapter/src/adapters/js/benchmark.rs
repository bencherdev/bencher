use bencher_json::{BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterJsBenchmark;

impl Adapter for AdapterJsBenchmark {
    fn parse(input: &str) -> Option<AdapterResults> {
        serde_json::from_str::<DotNet>(input)
            .ok()?
            .try_into()
            .ok()?
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DotNet {
    pub host_environment_info: JsonEmpty,
    pub benchmarks: Benchmarks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Benchmarks(pub Vec<Benchmark>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Benchmark {
    pub namespace: BenchmarkName,
    pub method: BenchmarkName,
    pub statistics: Statistics,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Statistics {
    #[serde(with = "rust_decimal::serde::float")]
    pub mean: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub standard_deviation: Decimal,
}

impl TryFrom<DotNet> for Option<AdapterResults> {
    type Error = AdapterError;

    fn try_from(dot_net: DotNet) -> Result<Self, Self::Error> {
        let benchmarks = dot_net.benchmarks.0;
        let mut benchmark_metrics = Vec::with_capacity(benchmarks.len());
        for benchmark in benchmarks {
            let Benchmark {
                namespace: mut benchmark_name,
                method,
                statistics,
            } = benchmark;
            let Statistics {
                mean,
                standard_deviation,
            } = statistics;

            benchmark_name.try_push('.', &method)?;

            // JSON output is always in nanos
            let units = Units::Nano;
            // The `Mode` is called `Throughput` but it appears to be measuring latency
            // https://benchmarkdotnet.org/articles/guides/choosing-run-strategy.html#throughput
            let value = latency_as_nanos(mean, units);
            let standard_deviation = latency_as_nanos(standard_deviation, units);
            let json_metric = JsonMetric {
                value,
                lower_bound: Some(value - standard_deviation),
                upper_bound: Some(value + standard_deviation),
            };

            benchmark_metrics.push((benchmark_name, json_metric));
        }

        Ok(AdapterResults::new_latency(benchmark_metrics))
    }
}

#[cfg(test)]
pub(crate) mod test_js_benchmark {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_latency},
        AdapterResults,
    };

    use super::AdapterJsBenchmark;

    fn convert_js_benchmark(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/js/benchmark/{suffix}.json");
        convert_file_path::<AdapterJsBenchmark>(&file_path)
    }

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_js_benchmark("two");
        validate_adapter_js_benchmark(results);
    }

    pub fn validate_adapter_js_benchmark(results: AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep10")
            .unwrap();
        validate_latency(
            metrics,
            10362283.085796878,
            Some(10316580.967427673),
            Some(10407985.204166083),
        );

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep20")
            .unwrap();
        validate_latency(
            metrics,
            20360791.931687497,
            Some(20312811.199369717),
            Some(20408772.664005276),
        );
    }
}
