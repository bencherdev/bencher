use clap::Parser;

use super::CliTuning;

/// Start the runner, polling for and executing benchmark jobs.
#[derive(Parser, Debug)]
pub struct CliUp {
    /// API server host URL.
    #[arg(long, env = "BENCHER_HOST", default_value = bencher_json::BENCHER_API_URL_STR)]
    pub host: url::Url,

    /// Runner authentication token.
    #[arg(long, env = "BENCHER_RUNNER_TOKEN")]
    pub token: bencher_json::Secret,

    /// Runner UUID or slug.
    #[arg(long, env = "BENCHER_RUNNER")]
    pub runner: String,

    /// Long-poll timeout in seconds (1-900).
    #[arg(long, default_value = "55", value_parser = clap::value_parser!(u32).range(1..=900))]
    pub poll_timeout: u32,

    #[command(flatten)]
    pub tuning: CliTuning,

    /// Maximum size in bytes for collected stdout/stderr (default: 25 MiB).
    #[arg(long)]
    pub max_output_size: Option<usize>,

    /// Maximum number of output files to decode (default: 255).
    #[arg(long)]
    pub max_file_count: Option<u32>,

    /// Grace period in seconds after exit code before final collection.
    #[arg(long)]
    pub grace_period: Option<bencher_runner::GracePeriod>,

    /// Firecracker process log level (default: warning).
    #[arg(long, default_value = "warning")]
    pub firecracker_log_level: bencher_runner::FirecrackerLogLevel,
}
