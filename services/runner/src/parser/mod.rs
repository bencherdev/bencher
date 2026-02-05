mod daemon;

use clap::{Parser, Subcommand};

pub use daemon::TaskDaemon;

#[derive(Parser, Debug)]
#[command(name = "bencher-runner")]
#[command(about = "Execute benchmarks in isolated Firecracker microVMs", long_about = None)]
pub struct TaskRunner {
    #[command(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Run as a daemon, polling for and executing benchmark jobs.
    Daemon(TaskDaemon),
    /// Pull image, create rootfs, and execute in isolated Firecracker microVM.
    Run(TaskRun),
}

/// Arguments for the `run` subcommand.
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI flags map to independent tuning knobs"
)]
#[derive(Parser, Debug)]
pub struct TaskRun {
    /// OCI image (local path or registry reference).
    #[arg(long)]
    pub image: String,

    /// JWT token for registry authentication.
    #[arg(long)]
    pub token: Option<String>,

    /// Number of vCPUs.
    #[arg(long, default_value = "1")]
    pub vcpus: u8,

    /// Memory in MiB.
    #[arg(long, default_value = "512")]
    pub memory: u32,

    /// Execution timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Output file path inside guest.
    #[arg(long)]
    pub output: Option<String>,

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
}
