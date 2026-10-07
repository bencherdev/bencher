pub mod server;

use bencher_json::{RunnerResourceId, Secret};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskRunnerOps {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Full provisioning: install OS, apply host settings, harden, and deploy runner
    Provision(TaskProvision),
    /// Configure kernel boot args for CPU isolation and reboot
    Isolate(TaskIsolate),
    /// Write every host setting that differs from the desired state, then report it
    Host(TaskHost),
    /// Download the runner binary of the runner's update channel and deploy it to a server
    Deploy(TaskDeploy),
    /// Show runner binary version
    Version(TaskVersion),
    /// Start the runner service
    Start(TaskStart),
    /// Stop the runner service
    Stop(TaskStop),
    /// View runner service logs
    Logs(TaskLogs),
    /// Check a runner's health and compare it against a reference runner (read-only)
    Audit(TaskAudit),
}

#[derive(Parser, Debug)]
pub struct TaskProvision {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Path to runner binary to deploy (Linux `x86_64`)
    #[clap(long)]
    pub runner_binary: Option<Utf8PathBuf>,
}

#[derive(Parser, Debug)]
pub struct TaskIsolate {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Benchmark CPU list to isolate (e.g. `1-5`; defaults to the lowest logical CPU of each physical core except CPU 0's)
    #[clap(long)]
    pub cpus: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TaskHost {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TaskDeploy {
    /// Runner slug or UUID
    pub runner: RunnerResourceId,

    /// IP address or hostname of the server
    #[clap(long)]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long)]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner authentication key
    #[clap(long)]
    pub key: Option<Secret>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,

    /// Update channel for automatic updates and the release to deploy (stable or canary)
    #[clap(long)]
    pub update_channel: Option<bencher_json::UpdateChannel>,

    /// CI run ID of a `devel` push run to deploy instead of the update channel's release
    #[clap(long)]
    pub run_id: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct TaskVersion {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TaskStart {
    /// Runner slug or UUID
    pub runner: RunnerResourceId,

    /// IP address or hostname of the server
    #[clap(long)]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long)]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner authentication key
    #[clap(long)]
    pub key: Option<Secret>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,

    /// Update channel for automatic updates (stable or canary)
    #[clap(long)]
    pub update_channel: Option<bencher_json::UpdateChannel>,

    /// Allow executing jobs without a sandbox (sets `BENCHER_DANGER_ALLOW_NO_SANDBOX=true`).
    #[clap(long)]
    pub danger_allow_no_sandbox: bool,
}

#[derive(Parser, Debug)]
pub struct TaskStop {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TaskLogs {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Number of lines to show (omit to show all logs)
    #[clap(long)]
    pub lines: Option<u32>,

    /// Follow logs in real-time
    #[clap(long)]
    pub follow: bool,
}

#[derive(Parser, Debug)]
pub struct TaskAudit {
    /// Runner slug or UUID (for runners.json lookup)
    pub runner: Option<RunnerResourceId>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "runner")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "runner")]
    pub ssh: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Reference runner slug or UUID to compare against (from runners.json)
    #[clap(long)]
    pub against: Option<RunnerResourceId>,
}
