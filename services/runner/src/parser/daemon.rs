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
    pub host: String,

    /// Runner authentication token.
    #[arg(long, env = "BENCHER_RUNNER_TOKEN")]
    pub token: String,

    /// Runner UUID or slug.
    #[arg(long, env = "BENCHER_RUNNER")]
    pub runner: String,

    /// Comma-separated labels for job matching.
    #[arg(long, env = "BENCHER_RUNNER_LABELS", value_delimiter = ',')]
    pub labels: Vec<String>,

    /// Long-poll timeout in seconds (max 60).
    #[arg(long, default_value = "55")]
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

    /// Maximum size in bytes for collected stdout/stderr (default: 10 MiB).
    #[arg(long)]
    pub max_output_size: Option<usize>,
}
