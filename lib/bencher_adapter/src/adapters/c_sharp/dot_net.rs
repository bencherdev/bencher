use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonAny, JsonNewMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adaptable, AdapterError, Settings,
};

pub struct AdapterCSharpDotNet;

impl Adaptable for AdapterCSharpDotNet {
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
    pub host_environment_info: JsonAny,
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
            let spread = latency_as_nanos(spread, units);
            let json_metric = JsonNewMetric {
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
pub(crate) mod test_c_sharp_dot_net {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{
            convert_file_path, convert_file_path_median, opt_convert_file_path, validate_latency,
        },
        AdapterResults, Settings,
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
        let two = "two";
        let file_path = file_path(two);

        let results = convert_c_sharp_dot_net(two);
        validate_adapter_c_sharp_dot_net(&results);

        let results = opt_convert_file_path::<AdapterCSharpDotNet>(
            &file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_c_sharp_dot_net(&results);

        let results = convert_c_sharp_dot_net_median(two);
        validate_adapter_c_sharp_dot_net_median(&results);
    }

    pub fn validate_adapter_c_sharp_dot_net(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep10")
            .unwrap();
        validate_latency(
            metrics,
            10_362_283.085_796_878,
            Some(10_316_580.967_427_673),
            Some(10_407_985.204_166_083),
        );

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep20")
            .unwrap();
        validate_latency(
            metrics,
            20_360_791.931_687_497,
            Some(20_312_811.199_369_717),
            Some(20_408_772.664_005_276),
        );
    }

    fn validate_adapter_c_sharp_dot_net_median(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep10")
            .unwrap();
        validate_latency(
            metrics,
            10_360_382.695_312_5,
            Some(10_305_689.064_843_748),
            Some(10_415_076.325_781_252),
        );

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep20")
            .unwrap();
        validate_latency(
            metrics,
            20_362_636.192_500_003,
            Some(20_283_296.531_562_5),
            Some(20_441_975.853_437_506),
        );
    }

    #[test]
    fn test_adapter_c_sharp_dot_net_two_more() {
        let results = convert_c_sharp_dot_net("two_more");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Sample.Fib10").unwrap();
        validate_latency(
            metrics,
            24.420_208_500_964_3,
            Some(24.222_087_247_885_93),
            Some(24.618_329_754_042_67),
        );

        let metrics = results.get("Sample.Fib20").unwrap();
        validate_latency(
            metrics,
            51.520_081_515_495_59,
            Some(50.729_707_813_342_635),
            Some(52.310_455_217_648_546),
        );
    }

    #[test]
    fn test_adapter_c_sharp_dot_net_two_more_median() {
        let results = convert_c_sharp_dot_net_median("two_more");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Sample.Fib10").unwrap();
        validate_latency(
            metrics,
            24.419_498_533_010_483,
            Some(24.247_002_340_853_214),
            Some(24.591_994_725_167_75),
        );

        let metrics = results.get("Sample.Fib20").unwrap();
        validate_latency(
            metrics,
            51.401_955_902_576_45,
            Some(50.498_313_307_762_146),
            Some(52.305_598_497_390_75),
        );
    }
}
