use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "bencher-runner")]
#[command(about = "Execute benchmarks in isolated VMs", long_about = None)]
pub struct TaskRunner {
    #[command(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Pull image, create rootfs, and execute in isolated VM.
    Run(TaskRun),
    /// Run VMM directly (internal, called by 'run').
    Vmm(TaskVmm),
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

/// Arguments for the `vmm` subcommand.
#[derive(Parser, Debug)]
pub struct TaskVmm {
    /// Path to the jail root directory.
    #[arg(long)]
    pub jail_root: String,

    /// Path to the kernel (relative to jail root after pivot).
    #[arg(long)]
    pub kernel: String,

    /// Path to the rootfs (relative to jail root after pivot).
    #[arg(long)]
    pub rootfs: String,

    /// Path to vsock socket (relative to jail root after pivot).
    #[arg(long)]
    pub vsock: Option<String>,

    /// Number of vCPUs.
    #[arg(long, default_value = "1")]
    pub vcpus: u8,

    /// Memory in MiB.
    #[arg(long, default_value = "512")]
    pub memory: u32,

    /// Execution timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Kernel command line.
    #[arg(long, default_value = "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda ro")]
    pub cmdline: String,
}
