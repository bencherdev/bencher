use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "bencher-runner")]
#[command(about = "Execute benchmarks in isolated Firecracker microVMs", long_about = None)]
pub struct TaskRunner {
    #[command(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Pull image, create rootfs, and execute in isolated Firecracker microVM.
    Run(TaskRun),
}

/// Arguments for the `run` subcommand.
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
}
