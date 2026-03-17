use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliNoise {
    /// Total measurement duration in seconds
    #[clap(long, default_value_t = 30)]
    pub duration: u64,

    /// Output format
    #[clap(long, default_value = "human")]
    pub format: CliNoiseFormat,

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
