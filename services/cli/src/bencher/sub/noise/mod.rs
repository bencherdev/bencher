use std::time::Duration;

use crate::{
    CliError, cli_eprintln_quietable, cli_println,
    parser::noise::{CliNoise, CliNoiseFormat},
};

use super::SubCmd;

mod benchmark;
mod bmf;
mod platform;
mod report;
mod score;

use benchmark::{cache, compute, io};

#[derive(Debug, Clone)]
pub struct Noise {
    duration: u64,
    format: NoiseFormat,
    quiet: bool,
}

#[derive(Debug, Clone, Copy)]
enum NoiseFormat {
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

impl From<CliNoise> for Noise {
    fn from(cli: CliNoise) -> Self {
        let CliNoise {
            duration,
            format,
            quiet,
        } = cli;
        Self {
            duration,
            format: match format {
                CliNoiseFormat::Human => NoiseFormat::Human,
                CliNoiseFormat::Json => NoiseFormat::Json,
            },
            quiet,
        }
    }
}

impl SubCmd for Noise {
    async fn exec(&self) -> Result<(), CliError> {
        self.exec_inner().map_err(Into::into)
    }
}

impl Noise {
    fn exec_inner(&self) -> Result<(), NoiseError> {
        let log = !self.quiet;
        let total_duration = Duration::from_secs(self.duration);

        // Allocate time: 30% compute, 30% cache, 20% I/O, 20% platform metrics
        let compute_duration = total_duration * 3 / 10;
        let cache_duration = total_duration * 3 / 10;
        let io_duration = total_duration * 2 / 10;
        let platform_duration = total_duration * 2 / 10;

        cli_eprintln_quietable!(
            log,
            "Measuring environment noise ({} seconds)...",
            self.duration
        );

        // Detect cache sizes first (for the cache benchmark)
        let cache_sizes = platform::detect_cache_sizes();
        let l3_size = cache_sizes.l3;

        // Run benchmarks
        cli_eprintln_quietable!(log, "  Running compute jitter benchmark...");
        let compute_result = compute::run_compute_benchmark(compute_duration);
        cli_eprintln_quietable!(log, "    CoV: {:.1}%", compute_result.cov_percent);

        cli_eprintln_quietable!(log, "  Running cache jitter benchmark...");
        let cache_result = cache::run_cache_benchmark(cache_duration, l3_size);
        cli_eprintln_quietable!(log, "    CoV: {:.1}%", cache_result.cov_percent);

        cli_eprintln_quietable!(log, "  Running I/O jitter benchmark...");
        let io_result = io::run_io_benchmark(io_duration)?;
        cli_eprintln_quietable!(log, "    CoV: {:.1}%", io_result.cov_percent);

        cli_eprintln_quietable!(log, "  Collecting platform metrics...");
        let platform_metrics = platform::collect_metrics(platform_duration);

        // Calculate composite score
        let noise_score = score::calculate_noise_score(
            &compute_result,
            &cache_result,
            &io_result,
            &platform_metrics,
        );

        // Output results
        match self.format {
            NoiseFormat::Human => {
                let output = report::format_report(
                    self.duration,
                    &compute_result,
                    &cache_result,
                    &io_result,
                    &platform_metrics,
                    noise_score,
                );
                cli_println!("{output}");
            },
            NoiseFormat::Json => {
                let bmf_results = bmf::build_bmf(
                    &compute_result,
                    &cache_result,
                    &io_result,
                    &platform_metrics,
                    noise_score,
                )?;
                cli_println!(
                    "{}",
                    serde_json::to_string_pretty(&bmf_results)
                        .map_err(NoiseError::SerializeResults)?
                );
            },
        }

        Ok(())
    }
}
