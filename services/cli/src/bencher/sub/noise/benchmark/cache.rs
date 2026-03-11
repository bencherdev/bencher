use std::time::{Duration, Instant};

use super::BenchmarkResult;

const DEFAULT_L3_SIZE: usize = 8 * 1024 * 1024; // 8MB default
const CACHE_LINE_SIZE: usize = 64;

/// Cache-sensitive memory traversal benchmark.
/// Allocates an array sized to ~75% of L3 cache and traverses it sequentially.
/// In a quiet environment, the array stays in cache and traversals are fast and consistent.
/// Noisy neighbors cause cache evictions, increasing variance.
pub fn run_cache_benchmark(duration: Duration, l3_size: Option<usize>) -> BenchmarkResult {
    let cache_size = l3_size.unwrap_or(DEFAULT_L3_SIZE);
    // Use 75% of L3 cache, rounded to cache line boundary
    #[expect(clippy::integer_division)]
    let array_bytes = cache_size * 3 / 4;
    #[expect(clippy::integer_division)]
    let array_size = array_bytes / CACHE_LINE_SIZE * CACHE_LINE_SIZE;
    #[expect(clippy::integer_division)]
    let num_elements = array_size / size_of::<u64>();

    // Allocate and initialize the array
    let mut data: Vec<u64> = vec![1; num_elements];

    let mut samples = Vec::new();
    let start = Instant::now();

    while start.elapsed() < duration {
        let iter_start = Instant::now();
        // Sequential traversal with writes to prevent optimization
        let mut acc: u64 = 0;
        for element in &mut data {
            acc = acc.wrapping_add(*element);
            *element = acc;
        }
        std::hint::black_box(acc);
        let elapsed = iter_start.elapsed();
        samples.push(elapsed.as_secs_f64() * 1e9);
    }

    BenchmarkResult::from_samples(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_benchmark_produces_samples() {
        let result = run_cache_benchmark(Duration::from_millis(100), Some(256 * 1024));
        assert!(result.mean_ns > 0.0);
        assert!(result.iterations > 0);
    }

    #[test]
    fn cache_benchmark_default_l3() {
        let result = run_cache_benchmark(Duration::from_millis(50), None);
        assert!(result.iterations > 0);
    }
}
