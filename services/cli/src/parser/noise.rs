use bencher_json::BmfVersion;
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliNoise {
    /// Total measurement duration in seconds
    #[clap(long, default_value_t = 30)]
    pub duration: u64,

    /// Output format
    #[clap(long, default_value = "human")]
    pub format: CliNoiseFormat,

    /// BMF version of the `json` output format.
    /// At BMF version 1, each jitter benchmark writes its jitter measure's coefficient of variation as `cov`,
    /// and a `latency` measure with `mean`, `std_dev`, and `p99`. CPU steal and the noise score write `value`.
    /// A project at BMF version 0 needs `--bmf-version 0`.
    #[clap(long, default_value_t = BmfVersion::V1)]
    pub bmf_version: BmfVersion,

    /// Suppress progress output, only print final result
    #[clap(long)]
    pub quiet: bool,
}

/// Output format for the noise report
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliNoiseFormat {
    /// Human-readable terminal output
    Human,
    /// BMF JSON output
    Json,
}
