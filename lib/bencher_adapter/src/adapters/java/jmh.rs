use bencher_json::{BenchmarkName, JsonEmpty, JsonMetric};
use nom::{
    bytes::complete::{tag, take_till1},
    character::complete::space1,
    combinator::{eof, map_res},
    sequence::tuple,
    IResult,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{
        latency_as_nanos, parse_benchmark_name, parse_f64, parse_u64, parse_units,
        throughput_as_nanos, NomError,
    },
    results::adapter_results::{AdapterMetricKind, AdapterResults},
    Adapter, AdapterError,
};

pub struct AdapterJavaJmh;

impl Adapter for AdapterJavaJmh {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        serde_json::from_str::<Jmh>(input)?.try_into()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Jmh(pub Vec<Benchmark>);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Benchmark {
    pub benchmark: BenchmarkName,
    pub primary_metric: PrimaryMetric,
    pub secondary_metrics: JsonEmpty,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryMetric {
    pub score: Decimal,
    pub score_error: Decimal,
    pub score_unit: String,
}

impl TryFrom<Jmh> for AdapterResults {
    type Error = AdapterError;

    fn try_from(jmh: Jmh) -> Result<Self, Self::Error> {
        println!("{jmh:#?}");

        let mut benchmark_metrics = Vec::with_capacity(jmh.0.len());
        for benchmark in jmh.0 {
            let Benchmark {
                benchmark: benchmark_name,
                primary_metric,
                ..
            } = benchmark;
            let PrimaryMetric {
                score,
                score_error,
                score_unit,
            } = primary_metric;

            // Latency
            let metric_kind = if let Some((unit, slash_op)) = score_unit.split_once("/op") {
                if !slash_op.is_empty() {
                    return Err(AdapterError::BenchmarkUnits);
                }

                let time_unit = unit.parse()?;
                let value = latency_as_nanos(score, time_unit);
                let variance = latency_as_nanos(score_error, time_unit);
                let json_metric = JsonMetric {
                    value,
                    lower_bound: Some(value - variance),
                    upper_bound: Some(value + variance),
                };
                AdapterMetricKind::Latency(json_metric)
                // Throughput
            } else if let Some((ops_slash, unit)) = score_unit.split_once("ops/") {
                if !ops_slash.is_empty() {
                    return Err(AdapterError::BenchmarkUnits);
                }

                let time_unit = unit.parse()?;
                let value = throughput_as_nanos(score, time_unit);
                let variance = throughput_as_nanos(score_error, time_unit);
                let json_metric = JsonMetric {
                    value,
                    lower_bound: Some(value - variance),
                    upper_bound: Some(value + variance),
                };
                AdapterMetricKind::Throughput(json_metric)
            } else {
                return Err(AdapterError::BenchmarkUnits);
            };

            benchmark_metrics.push((benchmark_name, metric_kind));
        }

        AdapterResults::new(benchmark_metrics)
    }
}

#[cfg(test)]
pub(crate) mod test_java_jmh {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_latency, validate_throughput},
        AdapterResults,
    };

    use super::AdapterJavaJmh;

    fn convert_java_jmh(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/java/jmh/{suffix}.json");
        convert_file_path::<AdapterJavaJmh>(&file_path)
    }

    #[test]
    fn test_adapter_java_jmh_latency() {
        let results = convert_java_jmh("latency");
        assert_eq!(results.inner.len(), 1);

        let metrics = results
            .get("org.openjdk.jmh.samples.JMHSample_01_HelloWorld.wellHelloThere")
            .unwrap();
        validate_latency(
            metrics,
            3.3762388731228186e18,
            Some(3.3619508873788826e18),
            Some(3.3905268588667546e18),
        );
    }

    #[test]
    fn test_adapter_java_jmh_throughput() {
        let results = convert_java_jmh("throughput");
        assert_eq!(results.inner.len(), 1);

        let metrics = results
            .get("org.openjdk.jmh.samples.JMHSample_01_HelloWorld.wellHelloThere")
            .unwrap();
        validate_throughput(
            metrics,
            3.3762388731228183,
            Some(3.361950887378882),
            Some(3.3905268588667545),
        );
    }

    #[test]
    fn test_adapter_java_jmh_six() {
        let results = convert_java_jmh("six");
        validate_adapter_java_jmh(results);
    }

    pub fn validate_adapter_java_jmh(results: AdapterResults) {
        assert_eq!(results.inner.len(), 6);

        let metrics = results
            .get("com.github.benmanes.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(metrics, 325.0, None, None);

        let metrics = results
            .get("com.github.benmanes.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(metrics, 40_537.123, None, None);

        let metrics = results
            .get("com.github.benmanes.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(metrics, 325.0, None, None);

        let metrics = results
            .get("com.github.benmanes.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(metrics, 40_537.123, None, None);

        let metrics = results
            .get("com.github.benmanes.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(metrics, 40_537.456, None, None);
    }
}
