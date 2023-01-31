use bencher_json::{BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterCSharpDotNet;

impl Adapter for AdapterCSharpDotNet {
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

    fn try_from(dotnet: DotNet) -> Result<Self, Self::Error> {
        let benchmarks = dotnet.benchmarks.0;
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

            benchmark_name.push('.', &method);

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
pub(crate) mod test_c_sharp_dotnet {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_latency},
        AdapterResults,
    };

    use super::AdapterCSharpDotNet;

    fn convert_c_sharp_dotnet(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/c_sharp/dotnet/{suffix}.json");
        convert_file_path::<AdapterCSharpDotNet>(&file_path)
    }

    #[test]
    fn test_adapter_c_sharp_dotnet_two() {
        let results = convert_c_sharp_dotnet("two");
        validate_adapter_c_sharp_dotnet(results);
    }

    pub fn validate_adapter_c_sharp_dotnet(results: AdapterResults) {
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

    #[test]
    fn test_adapter_c_sharp_dotnet_two_more() {
        let results = convert_c_sharp_dotnet("two_more");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Sample.Fib10").unwrap();
        validate_latency(
            metrics,
            24.4202085009643,
            Some(24.22208724788593),
            Some(24.61832975404267),
        );

        let metrics = results.get("Sample.Fib20").unwrap();
        validate_latency(
            metrics,
            51.52008151549559,
            Some(50.729707813342635),
            Some(52.310455217648546),
        );
    }
}
