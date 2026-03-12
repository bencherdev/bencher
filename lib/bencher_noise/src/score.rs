use super::{benchmark::BenchmarkResult, platform::PlatformMetrics};

/// Weight for compute jitter in the composite score.
const WEIGHT_COMPUTE: f64 = 0.30;
/// Weight for cache jitter in the composite score.
const WEIGHT_CACHE: f64 = 0.40;
/// Weight for I/O jitter in the composite score.
const WEIGHT_IO: f64 = 0.15;
/// Weight for CPU steal in the composite score.
const WEIGHT_STEAL: f64 = 0.15;

/// Noise level classification thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseLevel {
    Quiet,
    Moderate,
    Noisy,
    VeryNoisy,
}

impl NoiseLevel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s <= 40.0 => Self::Quiet,
            s if s <= 55.0 => Self::Moderate,
            s if s <= 70.0 => Self::Noisy,
            _ => Self::VeryNoisy,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "Quiet",
            Self::Moderate => "Moderate",
            Self::Noisy => "Noisy",
            Self::VeryNoisy => "Very Noisy",
        }
    }
}

/// Calculate the composite noise score (0-100 dB scale).
///
/// Uses `33 * log10(weighted_cov)` to map weighted `CoV` values to a 0-100 range,
/// producing scores that align with real-world sound perception (e.g. 5% `CoV` ≈ 23 dB whisper).
/// When CPU steal is unavailable, its weight is redistributed proportionally.
pub fn calculate_noise_score(
    compute: &BenchmarkResult,
    cache: &BenchmarkResult,
    io: &BenchmarkResult,
    platform: &PlatformMetrics,
) -> f64 {
    let steal = platform.cpu_steal_percent.unwrap_or(0.0);
    let has_steal = platform.cpu_steal_percent.is_some();

    // If steal is not available, redistribute its weight
    let (w_compute, w_cache, w_io, w_steal) = if has_steal {
        (WEIGHT_COMPUTE, WEIGHT_CACHE, WEIGHT_IO, WEIGHT_STEAL)
    } else {
        let redistribute = WEIGHT_STEAL / 3.0;
        (
            WEIGHT_COMPUTE + redistribute,
            WEIGHT_CACHE + redistribute,
            WEIGHT_IO + redistribute,
            0.0,
        )
    };

    let weighted_cov = compute.cov_percent * w_compute
        + cache.cov_percent * w_cache
        + io.cov_percent * w_io
        + steal * w_steal;

    log_scale(weighted_cov)
}

/// Map a weighted `CoV` value to the 0-100 dB scale using a logarithmic curve.
///
/// - `CoV` of 0% maps to score 0
/// - `CoV` of ~5% maps to score ~23 (whisper)
/// - `CoV` of ~10% maps to score ~33 (quiet library)
/// - `CoV` of ~25% maps to score ~46 (quiet room)
/// - `CoV` of ~50% maps to score ~56 (normal conversation)
/// - `CoV` of ~100% maps to score ~66 (busy office)
fn log_scale(weighted_cov: f64) -> f64 {
    if weighted_cov <= 0.0 {
        return 0.0;
    }
    // log10(cov) maps:
    //   1%    -> log10(1) = 0
    //   5%    -> log10(5) = 0.699
    //   10%   -> log10(10) = 1
    //   50%   -> log10(50) = 1.699
    //   100%  -> log10(100) = 2
    //   1000% -> log10(1000) = 3
    // Scale to 0-100 range: score = 33 * log10(cov)
    let raw = 33.0 * weighted_cov.log10();
    raw.clamp(0.0, 100.0)
}

/// Classify the noise level for a benchmark `CoV` percentage.
///
/// Benchmark `CoV` has wider thresholds than CPU steal because even
/// dedicated bare metal servers typically show 5-20% `CoV` for I/O workloads.
pub fn benchmark_cov_level(cov_percent: f64) -> NoiseLevel {
    match cov_percent {
        c if c <= 25.0 => NoiseLevel::Quiet,
        c if c <= 50.0 => NoiseLevel::Moderate,
        c if c <= 100.0 => NoiseLevel::Noisy,
        _ => NoiseLevel::VeryNoisy,
    }
}

/// Classify the noise level for a CPU steal percentage.
pub fn cov_level(cov_percent: f64) -> NoiseLevel {
    match cov_percent {
        c if c <= 1.0 => NoiseLevel::Quiet,
        c if c <= 5.0 => NoiseLevel::Moderate,
        c if c <= 15.0 => NoiseLevel::Noisy,
        _ => NoiseLevel::VeryNoisy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::CacheSizes;

    fn make_result(cov_percent: f64) -> BenchmarkResult {
        BenchmarkResult {
            iterations: 100,
            mean_ns: 1000.0,
            stddev_ns: cov_percent * 10.0,
            cov_percent,
            min_ns: 500.0,
            max_ns: 2000.0,
            p99_ns: 1800.0,
        }
    }

    #[test]
    fn noise_score_zero_cov() {
        let compute = make_result(0.0);
        let cache = make_result(0.0);
        let io = make_result(0.0);
        let platform = PlatformMetrics {
            cpu_steal_percent: Some(0.0),
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let score = calculate_noise_score(&compute, &cache, &io, &platform);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn noise_score_moderate() {
        let compute = make_result(2.0);
        let cache = make_result(3.0);
        let io = make_result(1.5);
        let platform = PlatformMetrics {
            cpu_steal_percent: Some(1.0),
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let score = calculate_noise_score(&compute, &cache, &io, &platform);
        assert!(score > 0.0);
        assert!(score < 80.0);
    }

    #[test]
    fn noise_score_high() {
        let compute = make_result(20.0);
        let cache = make_result(30.0);
        let io = make_result(15.0);
        let platform = PlatformMetrics {
            cpu_steal_percent: Some(10.0),
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        };
        let score = calculate_noise_score(&compute, &cache, &io, &platform);
        assert!(score > 40.0);
    }

    #[test]
    fn noise_score_no_steal_redistributes_weights() {
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
        let score = calculate_noise_score(&compute, &cache, &io, &platform);
        assert!(score > 0.0);
    }

    #[test]
    fn log_scale_clamped() {
        assert!((log_scale(0.0) - 0.0).abs() < f64::EPSILON);
        assert!(log_scale(1000.0) <= 100.0);
    }

    #[test]
    fn noise_level_thresholds() {
        assert_eq!(NoiseLevel::from_score(0.0), NoiseLevel::Quiet);
        assert_eq!(NoiseLevel::from_score(40.0), NoiseLevel::Quiet);
        assert_eq!(NoiseLevel::from_score(41.0), NoiseLevel::Moderate);
        assert_eq!(NoiseLevel::from_score(55.0), NoiseLevel::Moderate);
        assert_eq!(NoiseLevel::from_score(56.0), NoiseLevel::Noisy);
        assert_eq!(NoiseLevel::from_score(70.0), NoiseLevel::Noisy);
        assert_eq!(NoiseLevel::from_score(71.0), NoiseLevel::VeryNoisy);
    }

    #[test]
    fn cov_level_classification() {
        assert_eq!(cov_level(0.5), NoiseLevel::Quiet);
        assert_eq!(cov_level(3.0), NoiseLevel::Moderate);
        assert_eq!(cov_level(10.0), NoiseLevel::Noisy);
        assert_eq!(cov_level(20.0), NoiseLevel::VeryNoisy);
    }

    #[test]
    fn benchmark_cov_level_classification() {
        assert_eq!(benchmark_cov_level(5.0), NoiseLevel::Quiet);
        assert_eq!(benchmark_cov_level(18.0), NoiseLevel::Quiet);
        assert_eq!(benchmark_cov_level(25.0), NoiseLevel::Quiet);
        assert_eq!(benchmark_cov_level(30.0), NoiseLevel::Moderate);
        assert_eq!(benchmark_cov_level(50.0), NoiseLevel::Moderate);
        assert_eq!(benchmark_cov_level(75.0), NoiseLevel::Noisy);
        assert_eq!(benchmark_cov_level(100.0), NoiseLevel::Noisy);
        assert_eq!(benchmark_cov_level(150.0), NoiseLevel::VeryNoisy);
    }
}
