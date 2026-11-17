use bencher_adapter::{
    JsonV0Measures, JsonV0Results, JsonV1Measures, JsonV1Results, JsonV1Variant,
    results::adapter_metrics::NamedMap,
};
use bencher_json::{
    BenchmarkNameId, JsonNewMetric, MeasureNameId, MetricName, ParameterSet,
    project::measure::built_in::{BuiltInMeasure as _, default::Latency},
};

use super::{NoiseError, benchmark::BenchmarkResult, platform::PlatformMetrics};

const COMPUTE_JITTER: (&str, &str) = ("bencher::noise::compute_jitter", "compute-jitter");
const CACHE_JITTER: (&str, &str) = ("bencher::noise::cache_jitter", "cache-jitter");
const IO_JITTER: (&str, &str) = ("bencher::noise::io_jitter", "io-jitter");
const CPU_STEAL: (&str, &str) = ("bencher::noise::cpu_steal", "cpu-steal");
const COMPOSITE: (&str, &str) = ("bencher::noise::composite", "noise-score");

/// Build BMF v1 JSON output from noise measurement results.
pub fn build_bmf_v1(
    compute: &BenchmarkResult,
    cache: &BenchmarkResult,
    io: &BenchmarkResult,
    platform: &PlatformMetrics,
    noise_score: f64,
) -> Result<JsonV1Results, NoiseError> {
    let mut results = JsonV1Results::new();
    for ((benchmark, jitter), result) in [
        (COMPUTE_JITTER, compute),
        (CACHE_JITTER, cache),
        (IO_JITTER, io),
    ] {
        let BenchmarkResult {
            mean_ns,
            stddev_ns,
            cov_percent,
            p99_ns,
            ..
        } = *result;
        let mut measures = JsonV1Measures::new();
        measures.insert(measure_id(jitter)?, named([("cov", cov_percent)])?);
        measures.insert(
            Latency::name_id(),
            named([("mean", mean_ns), ("std_dev", stddev_ns), ("p99", p99_ns)])?,
        );
        insert_v1(&mut results, benchmark, measures)?;
    }
    let values = platform
        .cpu_steal_percent
        .map(|steal| (CPU_STEAL, steal))
        .into_iter()
        .chain([(COMPOSITE, noise_score)]);
    for ((benchmark, measure), value) in values {
        let mut measures = JsonV1Measures::new();
        measures.insert(measure_id(measure)?, named([("value", value)])?);
        insert_v1(&mut results, benchmark, measures)?;
    }
    Ok(results)
}

/// Build BMF v0 JSON output from noise measurement results.
pub fn build_bmf_v0(
    compute: &BenchmarkResult,
    cache: &BenchmarkResult,
    io: &BenchmarkResult,
    platform: &PlatformMetrics,
    noise_score: f64,
) -> Result<JsonV0Results, NoiseError> {
    let mut results = JsonV0Results::new();
    let jitter = [
        (COMPUTE_JITTER, compute.cov_percent),
        (CACHE_JITTER, cache.cov_percent),
        (IO_JITTER, io.cov_percent),
    ];
    let values = jitter.into_iter().chain(
        platform
            .cpu_steal_percent
            .map(|steal| (CPU_STEAL, steal))
            .into_iter()
            .chain([(COMPOSITE, noise_score)]),
    );
    for ((benchmark, measure), value) in values {
        let metric = JsonNewMetric {
            value: value.into(),
            lower_value: None,
            upper_value: None,
        };
        let mut measures = JsonV0Measures::new();
        measures.insert(measure_id(measure)?, metric);
        results.insert(benchmark_id(benchmark)?, measures);
    }
    Ok(results)
}

fn insert_v1(
    results: &mut JsonV1Results,
    benchmark: &str,
    measures: JsonV1Measures,
) -> Result<(), NoiseError> {
    results.insert(
        benchmark_id(benchmark)?,
        vec![JsonV1Variant {
            parameters: ParameterSet::default(),
            measures,
        }],
    );
    Ok(())
}

fn named<const N: usize>(metrics: [(&str, f64); N]) -> Result<NamedMap, NoiseError> {
    metrics
        .into_iter()
        .map(|(name, value)| {
            name.parse::<MetricName>()
                .map(|name| (name, value.into()))
                .map_err(NoiseError::ParseMetricName)
        })
        .collect()
}

fn benchmark_id(benchmark: &str) -> Result<BenchmarkNameId, NoiseError> {
    benchmark.parse().map_err(NoiseError::ParseBenchmarkName)
}

fn measure_id(measure: &str) -> Result<MeasureNameId, NoiseError> {
    measure.parse().map_err(NoiseError::ParseMeasureName)
}

#[cfg(test)]
mod tests {
    use bencher_adapter::{
        Adaptable as _, AdapterResults, Settings,
        adapters::json::{v0::AdapterJsonV0, v1::AdapterJsonV1},
    };
    use bencher_json::BmfVersion;

    use super::*;
    use crate::platform::CacheSizes;

    fn make_result(cov: f64) -> BenchmarkResult {
        BenchmarkResult {
            iterations: 100,
            mean_ns: 1000.0,
            stddev_ns: cov * 10.0,
            cov_percent: cov,
            p99_ns: 1800.0,
        }
    }

    fn platform(cpu_steal_percent: Option<f64>) -> PlatformMetrics {
        PlatformMetrics {
            cpu_steal_percent,
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        }
    }

    /// Every metric as `benchmark parameters measure name value`, sorted.
    fn metrics(results: &AdapterResults) -> Vec<String> {
        let mut metrics = results
            .inner
            .iter()
            .flat_map(|(benchmark, variants)| {
                variants.iter().flat_map(move |(parameters, measures)| {
                    measures.inner.iter().flat_map(move |(measure, metric)| {
                        metric.inner.iter().map(move |(name, value)| {
                            format!(
                                "{benchmark} {} {measure} {name} {value}",
                                parameters.canonical()
                            )
                        })
                    })
                })
            })
            .collect::<Vec<_>>();
        metrics.sort();
        metrics
    }

    /// Kills: the coefficient of variation written as `value` or to the `latency` measure,
    /// a timing statistic dropped or swapped with another, one benchmark's statistics written
    /// to another, and CPU steal or the score written with any name but `value`.
    #[test]
    fn build_bmf_v1_writes_cov_and_latency() {
        let compute = BenchmarkResult {
            iterations: 100,
            mean_ns: 100.0,
            stddev_ns: 3.0,
            cov_percent: 3.0,
            p99_ns: 110.0,
        };
        let cache = BenchmarkResult {
            iterations: 100,
            mean_ns: 200.0,
            stddev_ns: 16.0,
            cov_percent: 8.0,
            p99_ns: 250.0,
        };
        let io = BenchmarkResult {
            iterations: 100,
            mean_ns: 4000.0,
            stddev_ns: 480.0,
            cov_percent: 12.0,
            p99_ns: 6000.0,
        };
        let bmf = build_bmf_v1(&compute, &cache, &io, &platform(Some(4.5)), 68.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        let results = AdapterJsonV1::parse(&json, Settings::default()).unwrap();
        assert_eq!(
            metrics(&results),
            [
                "bencher::noise::cache_jitter {} cache-jitter cov 8",
                "bencher::noise::cache_jitter {} latency mean 200",
                "bencher::noise::cache_jitter {} latency p99 250",
                "bencher::noise::cache_jitter {} latency std_dev 16",
                "bencher::noise::composite {} noise-score value 68",
                "bencher::noise::compute_jitter {} compute-jitter cov 3",
                "bencher::noise::compute_jitter {} latency mean 100",
                "bencher::noise::compute_jitter {} latency p99 110",
                "bencher::noise::compute_jitter {} latency std_dev 3",
                "bencher::noise::cpu_steal {} cpu-steal value 4.5",
                "bencher::noise::io_jitter {} io-jitter cov 12",
                "bencher::noise::io_jitter {} latency mean 4000",
                "bencher::noise::io_jitter {} latency p99 6000",
                "bencher::noise::io_jitter {} latency std_dev 480",
            ]
        );
    }

    /// Kills: a CPU steal benchmark written when the platform has none.
    #[test]
    fn build_bmf_v1_no_steal() {
        let result = make_result(1.0);
        let bmf = build_bmf_v1(&result, &result, &result, &platform(None), 30.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        assert!(json.contains("bencher::noise::composite"));
        assert!(!json.contains("bencher::noise::cpu_steal"));
    }

    /// Kills: any change to the version 0 output.
    #[test]
    fn build_bmf_v0_writes_todays_output() {
        let bmf = build_bmf_v0(
            &make_result(3.2),
            &make_result(12.8),
            &make_result(8.1),
            &platform(Some(4.2)),
            68.0,
        )
        .unwrap();
        let json = serde_json::to_value(&bmf).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "bencher::noise::compute_jitter": {
                    "compute-jitter": { "value": 3.2, "lower_value": null, "upper_value": null }
                },
                "bencher::noise::cache_jitter": {
                    "cache-jitter": { "value": 12.8, "lower_value": null, "upper_value": null }
                },
                "bencher::noise::io_jitter": {
                    "io-jitter": { "value": 8.1, "lower_value": null, "upper_value": null }
                },
                "bencher::noise::cpu_steal": {
                    "cpu-steal": { "value": 4.2, "lower_value": null, "upper_value": null }
                },
                "bencher::noise::composite": {
                    "noise-score": { "value": 68.0, "lower_value": null, "upper_value": null }
                }
            })
        );
        let results =
            AdapterJsonV0::parse(&serde_json::to_string(&bmf).unwrap(), Settings::default());
        assert_eq!(results.map(|results| results.version), Some(BmfVersion::V0));
    }

    /// Kills: a CPU steal benchmark written at version 0 when the platform has none.
    #[test]
    fn build_bmf_v0_no_steal() {
        let result = make_result(1.0);
        let bmf = build_bmf_v0(&result, &result, &result, &platform(None), 30.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        assert!(json.contains("bencher::noise::composite"));
        assert!(!json.contains("bencher::noise::cpu_steal"));
    }
}
