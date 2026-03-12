use std::fmt::Write as _;

use super::{
    benchmark::BenchmarkResult,
    platform::{PlatformMetrics, VirtualizationType},
    score::{NoiseLevel, benchmark_cov_level, cpu_steal_level},
};

/// Format a human-readable noise report.
pub fn format_report(
    duration_secs: u64,
    compute: &BenchmarkResult,
    cache: &BenchmarkResult,
    io: &BenchmarkResult,
    platform: &PlatformMetrics,
    noise_score: f64,
) -> String {
    let level = NoiseLevel::from_score(noise_score);
    let mut report = String::new();

    report.push_str("Bencher Noise Report\n");
    report.push_str("========================================\n");

    // Platform info
    if let Some(true) = platform.is_virtualized {
        let vtype = platform
            .virtualization_type
            .map_or("Unknown", VirtualizationType::label);
        let _ = write!(report, "  Platform:          VM ({vtype})");
    } else {
        report.push_str("  Platform:          Native");
    }
    report.push('\n');
    let _ = write!(report, "  Duration:          {duration_secs}s");
    report.push('\n');

    // Cache sizes
    let l1d = format_cache_size(platform.cache_sizes.l1d);
    let l2 = format_cache_size(platform.cache_sizes.l2);
    let l3 = format_cache_size(platform.cache_sizes.l3);
    let _ = write!(
        report,
        "  CPU Caches:        L1d: {l1d} | L2: {l2} | L3: {l3}"
    );
    report.push('\n');
    report.push('\n');

    // Benchmark results
    format_benchmark_line(&mut report, "Compute Jitter", compute);
    format_benchmark_line(&mut report, "Cache Jitter", cache);
    format_benchmark_line(&mut report, "I/O Jitter", io);

    // CPU steal
    if let Some(steal) = platform.cpu_steal_percent {
        let steal_level = cpu_steal_level(steal);
        let bar = noise_bar(steal, 50.0);
        let label = steal_level.label();
        let _ = write!(report, "  CPU Steal:         {steal:.1}%  {bar}  {label}");
        report.push('\n');
    }

    // Context switches
    if let Some(rate) = platform.context_switch_rate {
        let _ = write!(report, "  Context Switches:  {rate:.0}/s");
        report.push('\n');
    }

    report.push('\n');
    let label = level.label();
    let _ = write!(report, "  Noise Score:       {noise_score:.0} dB  {label}");
    report.push('\n');
    report.push_str("========================================\n");

    // Advice
    let max_cov = compute
        .cov_percent
        .max(cache.cov_percent)
        .max(io.cov_percent);
    match level {
        NoiseLevel::Quiet => {
            report.push_str("  Your environment is quiet. Benchmark results should be reliable.\n");
        },
        NoiseLevel::Moderate => {
            let _ = write!(
                report,
                "  Your environment has moderate noise.\n  Benchmarks may vary +/-{max_cov:.1}% between runs."
            );
            report.push('\n');
        },
        NoiseLevel::Noisy | NoiseLevel::VeryNoisy => {
            let _ = write!(
                report,
                "  Your environment has significant benchmark noise.\n  Benchmarks here may vary +/-{max_cov:.1}% between runs."
            );
            report.push('\n');
        },
    }

    report
}

fn format_benchmark_line(report: &mut String, label: &str, result: &BenchmarkResult) {
    let level = benchmark_cov_level(result.cov_percent);
    let mean_us = result.mean_ns / 1000.0;
    let stddev_us = result.stddev_ns / 1000.0;
    let p99_us = result.p99_ns / 1000.0;
    let bar = noise_bar(result.cov_percent, 150.0);
    let level_label = level.label();
    let cov = result.cov_percent;
    let iters = result.iterations;
    let _ = write!(
        report,
        "  {label:<19}{cov:.1}% CoV  {bar}  {level_label}  ({iters} samples, mean {mean_us:.0}us, stddev {stddev_us:.0}us, p99 {p99_us:.0}us)"
    );
    report.push('\n');
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn noise_bar(value: f64, max: f64) -> String {
    let width: usize = 20;
    let filled = ((value / max) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}

#[expect(clippy::integer_division)]
fn format_cache_size(size: Option<usize>) -> String {
    match size {
        Some(s) if s >= 1024 * 1024 => format!("{}MB", s / (1024 * 1024)),
        Some(s) if s >= 1024 => format!("{}KB", s / 1024),
        Some(s) => format!("{s}B"),
        None => "N/A".to_owned(),
    }
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
            p99_ns: 1800.0,
        }
    }

    #[test]
    fn format_report_contains_sections() {
        let compute = make_result(3.0);
        let cache = make_result(5.0);
        let io = make_result(2.0);
        let platform = PlatformMetrics {
            cpu_steal_percent: Some(1.0),
            context_switch_rate: Some(5000.0),
            is_virtualized: Some(false),
            virtualization_type: None,
            cache_sizes: CacheSizes {
                l1d: Some(32 * 1024),
                l2: Some(256 * 1024),
                l3: Some(8 * 1024 * 1024),
            },
        };
        let report = format_report(60, &compute, &cache, &io, &platform, 45.0);
        assert!(report.contains("Bencher Noise Report"));
        assert!(report.contains("Native"));
        assert!(report.contains("60s"));
        assert!(report.contains("L1d: 32KB"));
        assert!(report.contains("L3: 8MB"));
        assert!(report.contains("Compute Jitter"));
        assert!(report.contains("Cache Jitter"));
        assert!(report.contains("CPU Steal"));
        assert!(report.contains("5000/s"));
        assert!(report.contains("45 dB"));
    }

    #[test]
    fn format_report_virtualized_docker() {
        let compute = make_result(3.0);
        let cache = make_result(5.0);
        let io = make_result(2.0);
        let platform = PlatformMetrics {
            cpu_steal_percent: None,
            context_switch_rate: None,
            is_virtualized: Some(true),
            virtualization_type: Some(VirtualizationType::Docker),
            cache_sizes: CacheSizes::default(),
        };
        let report = format_report(30, &compute, &cache, &io, &platform, 25.0);
        assert!(report.contains("VM (Docker)"));
    }

    #[test]
    fn cache_size_formatting() {
        assert_eq!(format_cache_size(Some(32 * 1024)), "32KB");
        assert_eq!(format_cache_size(Some(8 * 1024 * 1024)), "8MB");
        assert_eq!(format_cache_size(Some(512)), "512B");
        assert_eq!(format_cache_size(None), "N/A");
    }

    #[test]
    fn noise_bar_rendering() {
        let bar = noise_bar(15.0, 30.0);
        assert_eq!(bar.len(), 20);
        assert_eq!(bar, "##########----------");
    }
}
