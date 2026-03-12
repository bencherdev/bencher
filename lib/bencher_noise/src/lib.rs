mod benchmark;
mod bmf;
mod platform;
mod report;
mod score;

use std::time::Duration;

use benchmark::{cache, compute, io};

#[derive(Debug, Clone, Copy)]
pub enum NoiseFormat {
    Human,
    Json,
}

#[derive(thiserror::Error, Debug)]
pub enum NoiseError {
    #[error("Failed to run I/O benchmark: {0}")]
    Io(#[from] io::IoError),

    #[error("Failed to parse benchmark name: {0}")]
    ParseBenchmarkName(bencher_json::ValidError),

    #[error("Failed to parse measure name: {0}")]
    ParseMeasureName(bencher_json::ValidError),

    #[error("Failed to serialize noise results: {0}")]
    SerializeResults(serde_json::Error),
}

pub fn run_noise(
    duration: u64,
    format: NoiseFormat,
    quiet: bool,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> Result<(), NoiseError> {
    let log = !quiet;
    let total_duration = Duration::from_secs(duration);

    // Allocate time: 30% compute, 30% cache, 20% I/O, 20% platform metrics
    let compute_duration = total_duration * 3 / 10;
    let cache_duration = total_duration * 3 / 10;
    let io_duration = total_duration * 2 / 10;
    let platform_duration = total_duration * 2 / 10;

    if log {
        let _w = writeln!(
            stderr,
            "Measuring environment noise ({duration} seconds)..."
        );
    }

    // Detect cache sizes first (for the cache benchmark)
    let cache_sizes = platform::detect_cache_sizes();
    let l3_size = cache_sizes.l3;

    // Run benchmarks
    if log {
        let _w = writeln!(stderr, "  Running compute jitter benchmark...");
    }
    let compute_result = compute::run_compute_benchmark(compute_duration);
    if log {
        let _w = writeln!(stderr, "    CoV: {:.1}%", compute_result.cov_percent);
    }

    if log {
        let _w = writeln!(stderr, "  Running cache jitter benchmark...");
    }
    let cache_result = cache::run_cache_benchmark(cache_duration, l3_size);
    if log {
        let _w = writeln!(stderr, "    CoV: {:.1}%", cache_result.cov_percent);
    }

    if log {
        let _w = writeln!(stderr, "  Running I/O jitter benchmark...");
    }
    let io_result = io::run_io_benchmark(io_duration)?;
    if log {
        let _w = writeln!(stderr, "    CoV: {:.1}%", io_result.cov_percent);
    }

    if log {
        let _w = writeln!(stderr, "  Collecting platform metrics...");
    }
    let platform_metrics = platform::collect_metrics(platform_duration);

    // Calculate composite score
    let noise_score = score::calculate_noise_score(
        &compute_result,
        &cache_result,
        &io_result,
        &platform_metrics,
    );

    // Output results
    match format {
        NoiseFormat::Human => {
            let output = report::format_report(
                duration,
                &compute_result,
                &cache_result,
                &io_result,
                &platform_metrics,
                noise_score,
            );
            let _w = writeln!(stdout, "{output}");
        },
        NoiseFormat::Json => {
            let bmf_results = bmf::build_bmf(
                &compute_result,
                &cache_result,
                &io_result,
                &platform_metrics,
                noise_score,
            )?;
            let _w = writeln!(
                stdout,
                "{}",
                serde_json::to_string_pretty(&bmf_results).map_err(NoiseError::SerializeResults)?
            );
        },
    }

    Ok(())
}
