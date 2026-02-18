#[cfg(feature = "plus")]
mod up;

#[cfg(feature = "plus")]
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[cfg(feature = "plus")]
pub use up::CliUp;

#[derive(Parser, Debug)]
#[command(name = "runner")]
#[command(about = "Execute benchmarks in isolated Firecracker microVMs", long_about = None)]
pub struct CliRunner {
    #[command(subcommand)]
    pub sub: CliSub,
}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    #[cfg(feature = "plus")]
    /// Start the runner, polling for and executing benchmark jobs.
    Up(CliUp),
    #[cfg(feature = "plus")]
    /// Pull image, create rootfs, and execute in isolated Firecracker microVM.
    Run(CliRun),
}

/// Arguments for the `run` subcommand.
#[cfg(feature = "plus")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI flags map to independent tuning knobs"
)]
#[derive(Parser, Debug)]
pub struct CliRun {
    /// OCI image (local path or registry reference).
    #[arg(long)]
    pub image: String,

    /// JWT token for registry authentication.
    #[arg(long)]
    pub token: Option<String>,

    /// Number of vCPUs (overrides default for testing).
    #[arg(long)]
    pub vcpus: Option<u32>,

    /// Memory in MiB (overrides default for testing).
    #[arg(long)]
    pub memory: Option<u64>,

    /// Disk size in MiB (overrides default for testing).
    #[arg(long)]
    pub disk: Option<u64>,

    /// Execution timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Output file paths inside guest (may be repeated).
    #[arg(long)]
    pub output: Vec<Utf8PathBuf>,

    /// Maximum size in bytes for collected stdout/stderr (default: 25 MiB).
    #[arg(long)]
    pub max_output_size: Option<usize>,

    /// Maximum number of output files to decode (default: 255).
    #[arg(long)]
    pub max_file_count: Option<u32>,

    /// Container entrypoint override.
    #[arg(long, num_args = 1..=bencher_json::MAX_ENTRYPOINT_LEN)]
    pub entrypoint: Option<Vec<String>>,

    /// Container command override.
    #[arg(long, num_args = 1..=bencher_json::MAX_CMD_LEN)]
    pub cmd: Option<Vec<String>>,

    /// Environment variable in KEY=VALUE format (may be repeated).
    #[arg(long, value_parser = check_env)]
    pub env: Option<Vec<String>>,

    /// Enable network access in the VM.
    #[arg(long)]
    pub network: bool,

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

    /// Grace period in seconds after exit code before final collection (default: 1).
    #[arg(long, default_value = "1")]
    pub grace_period: bencher_runner::GracePeriod,

    /// Firecracker process log level (default: warning).
    #[arg(long, default_value = "warning")]
    pub firecracker_log_level: bencher_runner::FirecrackerLogLevel,
}

/// Validate that an environment variable argument is in `KEY=VALUE` format.
#[cfg(feature = "plus")]
fn check_env(arg: &str) -> Result<String, String> {
    let index = arg
        .find('=')
        .ok_or_else(|| format!("expected format `KEY=VALUE` but no `=` was found in: `{arg}`"))?;
    if index == 0 {
        return Err(format!(
            "expected format `KEY=VALUE` but no `KEY` was found in: `{arg}`"
        ));
    }
    Ok(arg.into())
}
