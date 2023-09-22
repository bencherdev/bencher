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
            3.376_238_873_122_818_6e18,
            Some(3.361_950_887_378_882_6e18),
            Some(3.390_526_858_866_754_6e18),
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
            3_376_238_873.122_818_5,
            Some(3_361_950_887.378_882_4),
            Some(3_390_526_858.866_754_5),
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
            152_520_132.344_021_95,
            Some(148_999_811.565_458_83),
            Some(156_040_453.122_585_06),
        );

        let metrics = results
            .get("com.github.guava.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(
            metrics,
            29_945_718.611_377_83,
            Some(28_668_756.962_039_28),
            Some(31_222_680.260_716_382),
        );

        let metrics = results
            .get("com.github.hashmap.caffeine.cache.ComputeBenchmark.compute_sameKey")
            .unwrap();
        validate_throughput(
            metrics,
            7_828_947.712_794_046,
            Some(-1_835_785.212_465_408_5),
            Some(17_493_680.638_053_5),
        );

        let metrics = results
            .get("com.github.caffeine.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            75_813_218.878_697_38,
            Some(69_632_899.287_084_84),
            Some(81_993_538.470_309_93),
        );

        let metrics = results
            .get("com.github.guava.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            32_709_984.763_771_25,
            Some(30_019_340.461_257_935),
            Some(35_400_629.066_284_57),
        );

        let metrics = results
            .get("com.github.hashmap.caffeine.cache.ComputeBenchmark.compute_spread")
            .unwrap();
        validate_throughput(
            metrics,
            113_640_916.672_629_92,
            Some(105_176_321.973_520_52),
            Some(122_105_511.371_739_3),
        );
    }
}
