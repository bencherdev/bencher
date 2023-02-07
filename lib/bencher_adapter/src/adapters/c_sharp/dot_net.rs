use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError, Settings,
};

pub struct AdapterCSharpDotNet;

impl Adapter for AdapterCSharpDotNet {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str::<DotNet>(input)
            .ok()?
            .convert(settings)
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
    #[serde(with = "rust_decimal::serde::float")]
    pub median: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub interquartile_range: Decimal,
}

impl DotNet {
    fn convert(self, settings: Settings) -> Result<Option<AdapterResults>, AdapterError> {
        let benchmarks = self.benchmarks.0;
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
                median,
                interquartile_range,
            } = statistics;

            benchmark_name.try_push('.', &method)?;

            // JSON output is always in nanos
            let units = Units::Nano;
            // The `Mode` is called `Throughput` but it appears to be measuring latency
            // https://benchmarkdotnet.org/articles/guides/choosing-run-strategy.html#throughput
            let (average, spread) = match settings.average.unwrap_or_default() {
                JsonAverage::Mean => (mean, standard_deviation),
                JsonAverage::Median => (median, interquartile_range),
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
pub(crate) mod test_c_sharp_dot_net {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, convert_file_path_median, validate_latency},
        AdapterResults,
    };

    use super::AdapterCSharpDotNet;

    fn convert_c_sharp_dot_net(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path::<AdapterCSharpDotNet>(&file_path)
    }

    fn convert_c_sharp_dot_net_median(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path_median::<AdapterCSharpDotNet>(&file_path)
    }

    fn file_path(suffix: &str) -> String {
        format!("./tool_output/c_sharp/dot_net/{suffix}.json")
    }

    #[test]
    fn test_adapter_c_sharp_dot_net_two() {
        let results = convert_c_sharp_dot_net("two");
        validate_adapter_c_sharp_dot_net(results);
    }

    pub fn validate_adapter_c_sharp_dot_net(results: AdapterResults) {
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
    fn test_adapter_c_sharp_dot_net_two_median() {
        let results = convert_c_sharp_dot_net_median("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep10")
            .unwrap();
        validate_latency(
            metrics,
            10360382.6953125,
            Some(10305689.064843748),
            Some(10415076.325781252),
        );

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep20")
            .unwrap();
        validate_latency(
            metrics,
            20362636.192500003,
            Some(20283296.5315625),
            Some(20441975.853437506),
        );
    }

    #[test]
    fn test_adapter_c_sharp_dot_net_two_more() {
        let results = convert_c_sharp_dot_net("two_more");
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

    #[test]
    fn test_adapter_c_sharp_dot_net_two_more_median() {
        let results = convert_c_sharp_dot_net_median("two_more");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Sample.Fib10").unwrap();
        validate_latency(
            metrics,
            24.419498533010483,
            Some(24.247002340853214),
            Some(24.59199472516775),
        );

        let metrics = results.get("Sample.Fib20").unwrap();
        validate_latency(
            metrics,
            51.40195590257645,
            Some(50.498313307762146),
            Some(52.30559849739075),
        );
    }
}
