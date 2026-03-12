use std::time::{Duration, Instant};

use super::BenchmarkResult;

/// Fixed-work CPU computation: iterative integer arithmetic.
/// Each iteration does identical work so variance reveals scheduling noise.
fn compute_iteration() -> u64 {
    let mut acc: u64 = 0x517c_c1b7_2722_0a95;
    for _ in 0..10_000 {
        acc = acc.wrapping_mul(6_364_136_223_846_793_005);
        acc = acc.wrapping_add(1_442_695_040_888_963_407);
        acc ^= acc >> 33;
    }
    // Use the result to prevent optimizer from eliminating the loop
    std::hint::black_box(acc)
}

pub fn run_compute_benchmark(duration: Duration) -> BenchmarkResult {
    let mut samples = Vec::new();
    let start = Instant::now();

    while start.elapsed() < duration {
        let iter_start = Instant::now();
        compute_iteration();
        let elapsed = iter_start.elapsed();
        samples.push(elapsed.as_secs_f64() * 1e9);
    }

    BenchmarkResult::from_samples(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_iteration_deterministic() {
        let a = compute_iteration();
        let b = compute_iteration();
        assert_eq!(a, b);
    }

    #[test]
    fn compute_benchmark_produces_samples() {
        let result = run_compute_benchmark(Duration::from_millis(100));
        assert!(result.mean_ns > 0.0);
        assert!(result.iterations > 0);
    }
}
