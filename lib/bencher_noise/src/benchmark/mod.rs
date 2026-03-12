pub mod cache;
pub mod compute;
pub mod io;

/// Result of running a synthetic benchmark.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub iterations: u64,
    pub mean_ns: f64,
    pub stddev_ns: f64,
    pub cov_percent: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    pub p99_ns: f64,
}

impl BenchmarkResult {
    /// Construct a `BenchmarkResult` from raw timing samples (in nanoseconds).
    /// The first 10% of samples are discarded as warmup.
    pub fn from_samples(mut samples: Vec<f64>) -> Self {
        // Discard first 10% as warmup
        #[expect(clippy::integer_division)]
        let warmup_count = samples.len() / 10;
        samples.drain(..warmup_count);

        if samples.is_empty() {
            return Self {
                iterations: 0,
                mean_ns: 0.0,
                stddev_ns: 0.0,
                cov_percent: 0.0,
                min_ns: 0.0,
                max_ns: 0.0,
                p99_ns: 0.0,
            };
        }

        let iterations = u64::try_from(samples.len()).unwrap_or(u64::MAX);
        let mean_ns = mean(&samples);
        let stddev_ns = stddev(&samples, mean_ns);
        let cov_percent = if mean_ns > 0.0 {
            (stddev_ns / mean_ns) * 100.0
        } else {
            0.0
        };
        let min_ns = samples.iter().copied().reduce(f64::min).unwrap_or_default();
        let max_ns = samples.iter().copied().reduce(f64::max).unwrap_or_default();
        let p99_ns = percentile(&mut samples, 99.0);

        Self {
            iterations,
            mean_ns,
            stddev_ns,
            cov_percent,
            min_ns,
            max_ns,
            p99_ns,
        }
    }
}

fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    #[expect(clippy::cast_precision_loss)]
    let len = samples.len() as f64;
    samples.iter().sum::<f64>() / len
}

fn stddev(samples: &[f64], mean: f64) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    #[expect(clippy::cast_precision_loss)]
    let len_minus_one = (samples.len() - 1) as f64;
    let variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / len_minus_one;
    variance.sqrt()
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(samples: &mut [f64], pct: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((pct / 100.0) * (samples.len() - 1) as f64).round() as usize;
    let idx = idx.min(samples.len() - 1);
    #[expect(clippy::indexing_slicing)]
    samples[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_calculation() {
        assert!((mean(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 3.0).abs() < f64::EPSILON);
        assert!((mean(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stddev_calculation() {
        let samples = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let m = mean(&samples);
        let sd = stddev(&samples, m);
        // Known sample stddev for this dataset is approximately 2.138
        assert!((sd - 2.138).abs() < 0.01);
    }

    #[test]
    fn percentile_calculation() {
        let mut samples: Vec<f64> = (1..=100).map(f64::from).collect();
        assert!((percentile(&mut samples, 99.0) - 99.0).abs() < 1.1);
        assert!((percentile(&mut samples, 50.0) - 50.0).abs() < 1.1);
    }

    #[test]
    fn from_samples_warmup_discard() {
        // 100 samples, first 10 should be discarded
        let samples: Vec<f64> = (0..100).map(|i| f64::from(i + 1) * 1000.0).collect();
        let result = BenchmarkResult::from_samples(samples);
        assert_eq!(result.iterations, 90);
    }

    #[test]
    fn from_samples_empty() {
        let result = BenchmarkResult::from_samples(Vec::new());
        assert_eq!(result.iterations, 0);
        assert!((result.mean_ns - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cov_calculation() {
        // Constant samples produce CoV = 0
        let samples = vec![100.0; 20];
        let result = BenchmarkResult::from_samples(samples);
        assert!((result.cov_percent - 0.0).abs() < f64::EPSILON);
    }
}
