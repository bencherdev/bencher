use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonEmpty, JsonMetric};

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, throughput_as_secs},
    results::adapter_results::{AdapterMetricKind, AdapterResults},
    Adapter, AdapterError, Settings,
};

pub struct AdapterJavaJmh;

impl Adapter for AdapterJavaJmh {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        serde_json::from_str::<Jmh>(input).ok()?.try_into().ok()?
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
    #[serde(with = "rust_decimal::serde::float")]
    pub score: Decimal,
    pub score_confidence: ScoreConfidence,
    pub score_unit: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreConfidence(
    #[serde(with = "rust_decimal::serde::float")] Decimal,
    #[serde(with = "rust_decimal::serde::float")] Decimal,
);

impl TryFrom<Jmh> for Option<AdapterResults> {
    type Error = AdapterError;

    fn try_from(jmh: Jmh) -> Result<Self, Self::Error> {
        let mut benchmark_metrics = Vec::with_capacity(jmh.0.len());
        for benchmark in jmh.0 {
            let Benchmark {
                benchmark: benchmark_name,
                primary_metric,
                ..
            } = benchmark;
            let PrimaryMetric {
                score,
                score_confidence,
                score_unit,
            } = primary_metric;

            let metric_kind = if let Some((unit, slash_op)) = score_unit.split_once("/op") {
                if !slash_op.is_empty() {
                    return Err(AdapterError::BenchmarkUnits(slash_op.into()));
                }

                let time_unit = unit.parse()?;
                let value = latency_as_nanos(score, time_unit);
                let lower_bound = latency_as_nanos(score_confidence.0, time_unit);
                let upper_bound = latency_as_nanos(score_confidence.1, time_unit);
                let json_metric = JsonMetric {
                    value,
                    lower_bound: Some(lower_bound),
                    upper_bound: Some(upper_bound),
                };
                AdapterMetricKind::Latency(json_metric)
            } else if let Some((ops_slash, unit)) = score_unit.split_once("ops/") {
                if !ops_slash.is_empty() {
                    return Err(AdapterError::BenchmarkUnits(ops_slash.into()));
                }

                let time_unit = unit.parse()?;
                let value = throughput_as_secs(score, time_unit);
                let lower_bound = throughput_as_secs(score_confidence.0, time_unit);
                let upper_bound = throughput_as_secs(score_confidence.1, time_unit);
                let json_metric = JsonMetric {
                    value,
                    lower_bound: Some(lower_bound),
                    upper_bound: Some(upper_bound),
                };
                AdapterMetricKind::Throughput(json_metric)
            } else {
                return Err(AdapterError::BenchmarkUnits(score_unit));
            };

            benchmark_metrics.push((benchmark_name, metric_kind));
        }

        Ok(AdapterResults::new(benchmark_metrics))
    }
}

#[cfg(test)]
pub(crate) mod test_java_jmh {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{
            convert_file_path, opt_convert_file_path, validate_latency, validate_throughput,
        },
        AdapterResults, Settings,
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
            3376238873.1228185,
            Some(3361950887.3788824),
            Some(3390526858.8667545),
        );
    }

    #[test]
    fn test_adapter_java_jmh_average() {
        let file_path = "./tool_output/java/jmh/six.json";
        let results = opt_convert_file_path::<AdapterJavaJmh>(
            file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_java_jmh(results);

        assert_eq!(
            None,
            opt_convert_file_path::<AdapterJavaJmh>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Median)
                }
            )
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
            .get("com.github.caffeine.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(
            metrics,
            152520132.34402195,
            Some(148999811.56545883),
            Some(156040453.12258506),
        );

        let metrics = results
            .get("com.github.guava.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(
            metrics,
            29945718.61137783,
            Some(28668756.96203928),
            Some(31222680.260716382),
        );

        let metrics = results
            .get("com.github.hashmap.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(
            metrics,
            7828947.712794046,
            Some(-1835785.2124654085),
            Some(17493680.6380535),
        );

        let metrics = results
            .get("com.github.caffeine.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            75813218.87869738,
            Some(69632899.28708484),
            Some(81993538.47030993),
        );

        let metrics = results
            .get("com.github.guava.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            32709984.76377125,
            Some(30019340.461257935),
            Some(35400629.06628457),
        );

        let metrics = results
            .get("com.github.hashmap.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            113640916.67262992,
            Some(105176321.97352052),
            Some(122105511.3717393),
        );
    }
}
