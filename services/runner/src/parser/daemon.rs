use clap::Parser;

#[cfg(debug_assertions)]
const DEFAULT_HOST: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
const DEFAULT_HOST: &str = "https://api.bencher.dev";

/// Run as a daemon, polling for and executing benchmark jobs.
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI flags map to independent tuning knobs"
)]
#[derive(Parser, Debug)]
pub struct TaskDaemon {
    /// API server host URL.
    #[arg(long, env = "BENCHER_HOST", default_value = DEFAULT_HOST)]
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

    // --- Host tuning flags ---
    /// Disable all host tuning optimizations.
    #[arg(long)]
    pub no_tuning: bool,

    /// Keep ASLR enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub aslr: bool,

    /// Keep NMI watchdog enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub nmi_watchdog: bool,

    /// Keep SMT / hyper-threading enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub smt: bool,

    /// Keep turboboost enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub turbo: bool,

    /// Set swappiness value (default: 10).
    #[arg(long)]
    pub swappiness: Option<u32>,

    /// Set CPU scaling governor (default: performance).
    #[arg(long)]
    pub governor: Option<String>,

    /// Set `perf_event_paranoid` value (default: -1).
    #[arg(long, allow_hyphen_values = true)]
    pub perf_event_paranoid: Option<i32>,

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
