use bencher_json::{BenchmarkName, JsonAny, JsonNewMetric, project::report::JsonAverage};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    Adaptable, AdapterError, Settings,
    adapters::util::{Units, latency_as_nanos},
    results::adapter_results::AdapterResults,
};
use crate::results::adapter_results::DotNetMeasure;

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
    pub namespace: Option<BenchmarkName>,
    pub method: BenchmarkName,
    pub statistics: Statistics,
    pub memory: Option<Memory>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Memory {
    pub gen0_collections: u32,
    pub gen1_collections: u32,
    pub gen2_collections: u32,
    pub total_operations: u32,
    pub bytes_allocated_per_operation: u32,
}

impl DotNet {
    fn convert(self, settings: Settings) -> Result<Option<AdapterResults>, AdapterError> {
        let benchmarks = self.benchmarks.0;
        let mut benchmark_metrics = Vec::with_capacity(benchmarks.len());
        for benchmark in benchmarks {
            let Benchmark {
                namespace,
                method,
                statistics,
                memory,
            } = benchmark;

            let Statistics {
                mean,
                standard_deviation,
                median,
                interquartile_range,
            } = statistics;

            let benchmark_name = match namespace {
                Some(mut name) => {
                    name.try_push('.', &method)?;
                    name
                },
                None => method,
            };

            // JSON output is always in nanos
            let units = Units::Nano;
            // The `Mode` is called `Throughput` but it appears to be measuring latency
            // https://benchmarkdotnet.org/articles/guides/choosing-run-strategy.html#throughput
            let (average, spread) = match settings.average.unwrap_or_default() {
                JsonAverage::Mean => (mean, standard_deviation),
                JsonAverage::Median => (median, interquartile_range),
            };
            let latency_value = latency_as_nanos(average, units);
            let spread = latency_as_nanos(spread, units);
            let json_latency_metric = JsonNewMetric {
                value: latency_value,
                lower_value: Some(latency_value - spread),
                upper_value: Some(latency_value + spread),
            };

            let latency_measure = DotNetMeasure::Latency(json_latency_metric);

            let mut measures = vec![latency_measure];

            if let Some(m) = memory {
                let allocated_json = JsonNewMetric {
                    value: m.bytes_allocated_per_operation.into(),
                    lower_value: None,
                    upper_value: None,
                };

                let allocated_measure = DotNetMeasure::Allocated(allocated_json);

                measures.push(allocated_measure);

                let gen0_collects_json = JsonNewMetric {
                    value: m.gen0_collections.into(),
                    lower_value: None,
                    upper_value: None,
                };

                let gen0_measure = DotNetMeasure::Gen0Collects(gen0_collects_json);

                measures.push(gen0_measure);

                let gen1_collects_json = JsonNewMetric {
                    value: m.gen1_collections.into(),
                    lower_value: None,
                    upper_value: None,
                };

                let gen1_measure = DotNetMeasure::Gen1Collects(gen1_collects_json);

                measures.push(gen1_measure);

                let gen2_collects_json = JsonNewMetric {
                    value: m.gen2_collections.into(),
                    lower_value: None,
                    upper_value: None,
                };

                let gen2_measure = DotNetMeasure::Gen2Collects(gen2_collects_json);

                measures.push(gen2_measure);

                let total_operations_json = JsonNewMetric {
                    value: m.total_operations.into(),
                    lower_value: None,
                    upper_value: None,
                };

                let total_operations_measure = DotNetMeasure::TotalOperations(total_operations_json);

                measures.push(total_operations_measure);
            }

            benchmark_metrics.push((benchmark_name, measures));
        }

        Ok(AdapterResults::new_dotnet(benchmark_metrics))
    }
}

#[cfg(test)]
pub(crate) mod test_c_sharp_dot_net {
    use ordered_float::OrderedFloat;
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{
            convert_file_path, convert_file_path_median, opt_convert_file_path, validate_latency,
        },
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
    fn adapter_c_sharp_dot_net_null_namespace() {
        let null_namespace = "null_namespace";
        let file_path = file_path(null_namespace);

        let results = convert_c_sharp_dot_net(null_namespace);
        validate_adapter_c_sharp_dot_net_null_namespace(&results);

        let results = opt_convert_file_path::<AdapterCSharpDotNet>(
            &file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();

        validate_adapter_c_sharp_dot_net_null_namespace(&results);
    }

    fn validate_adapter_c_sharp_dot_net_null_namespace(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 1);
        assert!(results.get("NullNamespaceMethod").is_some());
    }

    #[test]
    fn adapter_c_sharp_dot_net_two() {
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
    fn adapter_c_sharp_dot_net_two_more() {
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
    fn adapter_c_sharp_dot_net_memory() {
        let results = convert_c_sharp_dot_net("memory");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("BenchmarkDotNet.Samples.AllocEmptyList").unwrap();

        let allocated = metrics.get("allocated").unwrap();
        assert_eq!(allocated.value, OrderedFloat::from(368));

        let gen0collects = metrics.get("gen0-collects").unwrap();
        assert_eq!(gen0collects.value, OrderedFloat::from(184));

        let gen1collects = metrics.get("gen1-collects").unwrap();
        assert_eq!(gen1collects.value, OrderedFloat::from(0));

        let gen2collects = metrics.get("gen2-collects").unwrap();
        assert_eq!(gen2collects.value, OrderedFloat::from(0));

        let total_operations = metrics.get("total-operations").unwrap();
        assert_eq!(total_operations.value, OrderedFloat::from(8388608));

        let latency = metrics.get("latency").unwrap();
        assert_eq!(latency.value, OrderedFloat::from(77.494_494_120_279_952));
        assert_eq!(latency.lower_value, Some(76.318_534_543_028_989).map(OrderedFloat::from));
        assert_eq!(latency.upper_value, Some(78.670_453_697_530_917).map(OrderedFloat::from));
    }

    #[test]
    fn adapter_c_sharp_dot_net_two_more_median() {
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
