use std::collections::HashMap;

use bencher_adapter::{AdapterResults, results::adapter_metrics::AdapterMetrics};
use bencher_json::{BenchmarkNameId, JsonNewMetric, MeasureNameId};

use super::{NoiseError, benchmark::BenchmarkResult, platform::PlatformMetrics};

/// Build BMF JSON output from noise measurement results.
pub fn build_bmf(
    compute: &BenchmarkResult,
    cache: &BenchmarkResult,
    io: &BenchmarkResult,
    platform: &PlatformMetrics,
    noise_score: f64,
) -> Result<AdapterResults, NoiseError> {
    let mut results = HashMap::new();

    // Compute jitter: CoV as value, min/max as bounds
    insert_measure(
        &mut results,
        "bencher::noise::compute_jitter",
        "compute-jitter",
        compute,
    )?;

    // Cache jitter
    insert_measure(
        &mut results,
        "bencher::noise::cache_jitter",
        "cache-jitter",
        cache,
    )?;

    // I/O jitter
    insert_measure(&mut results, "bencher::noise::io_jitter", "io-jitter", io)?;

    // CPU steal (if available)
    if let Some(steal) = platform.cpu_steal_percent {
        let metric = JsonNewMetric {
            value: steal.into(),
            lower_value: None,
            upper_value: None,
        };
        let measure_id: MeasureNameId =
            "cpu-steal".parse().map_err(NoiseError::ParseMeasureName)?;
        let benchmark_id: BenchmarkNameId = "bencher::noise::cpu_steal"
            .parse()
            .map_err(NoiseError::ParseBenchmarkName)?;
        let mut measures = HashMap::new();
        measures.insert(measure_id, metric);
        results.insert(benchmark_id, AdapterMetrics { inner: measures });
    }

    // Composite noise score
    {
        let metric = JsonNewMetric {
            value: noise_score.into(),
            lower_value: None,
            upper_value: None,
        };
        let measure_id: MeasureNameId = "noise-score"
            .parse()
            .map_err(NoiseError::ParseMeasureName)?;
        let benchmark_id: BenchmarkNameId = "bencher::noise::composite"
            .parse()
            .map_err(NoiseError::ParseBenchmarkName)?;
        let mut measures = HashMap::new();
        measures.insert(measure_id, metric);
        results.insert(benchmark_id, AdapterMetrics { inner: measures });
    }

    Ok(AdapterResults::from(results))
}

fn insert_measure(
    results: &mut HashMap<BenchmarkNameId, AdapterMetrics>,
    benchmark_name: &str,
    measure_slug: &str,
    bench_result: &BenchmarkResult,
) -> Result<(), NoiseError> {
    let metric = JsonNewMetric {
        value: bench_result.cov_percent.into(),
        lower_value: Some(bench_result.min_ns.into()),
        upper_value: Some(bench_result.max_ns.into()),
    };
    let measure_id: MeasureNameId = measure_slug.parse().map_err(NoiseError::ParseMeasureName)?;
    let benchmark_id: BenchmarkNameId = benchmark_name
        .parse()
        .map_err(NoiseError::ParseBenchmarkName)?;
    let mut measures = HashMap::new();
    measures.insert(measure_id, metric);
    results.insert(benchmark_id, AdapterMetrics { inner: measures });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::CacheSizes;

    fn make_result(cov: f64) -> BenchmarkResult {
        BenchmarkResult {
            iterations: 100,
            mean_ns: 1000.0,
            stddev_ns: cov * 10.0,
            cov_percent: cov,
            min_ns: 500.0,
            max_ns: 2000.0,
            p99_ns: 1800.0,
        }
    }

    #[test]
    fn build_bmf_valid_json() {
        let compute = make_result(3.2);
        let cache = make_result(12.8);
        let io = make_result(8.1);
        let platform = PlatformMetrics {
            cpu_steal_percent: Some(4.2),
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let bmf = build_bmf(&compute, &cache, &io, &platform, 68.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        assert!(json.contains("bencher::noise::compute_jitter"));
        assert!(json.contains("bencher::noise::cache_jitter"));
        assert!(json.contains("bencher::noise::io_jitter"));
        assert!(json.contains("bencher::noise::cpu_steal"));
        assert!(json.contains("bencher::noise::composite"));
        assert!(json.contains("compute-jitter"));
        assert!(json.contains("noise-score"));
    }

    #[test]
    fn build_bmf_no_steal() {
        let compute = make_result(1.0);
        let cache = make_result(2.0);
        let io = make_result(1.5);
        let platform = PlatformMetrics {
            cpu_steal_percent: None,
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let bmf = build_bmf(&compute, &cache, &io, &platform, 30.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        assert!(!json.contains("bencher::noise::cpu_steal"));
        assert!(json.contains("bencher::noise::composite"));
    }

    #[test]
    fn build_bmf_roundtrip() {
        let compute = make_result(5.0);
        let cache = make_result(5.0);
        let io = make_result(5.0);
        let platform = PlatformMetrics {
            cpu_steal_percent: None,
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let bmf = build_bmf(&compute, &cache, &io, &platform, 50.0).unwrap();
        let json = serde_json::to_string_pretty(&bmf).unwrap();
        // Verify it parses back as valid JSON
        drop(serde_json::from_str::<serde_json::Value>(&json).unwrap());
    }
}
