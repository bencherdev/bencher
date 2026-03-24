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
    /// Full provisioning: install OS, harden, and deploy runner
    Provision(TaskProvision),
    /// Download latest runner binary from CI and deploy to server
    Deploy(TaskDeploy),
    /// Start the runner service
    Start(TaskStart),
    /// Stop the runner service
    Stop(TaskStop),
    /// View runner service logs
    Logs(TaskLogs),
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
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Path to runner binary to deploy (Linux `x86_64`)
    #[clap(long)]
    pub runner_binary: Option<Utf8PathBuf>,
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
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner authentication token
    #[clap(long)]
    pub token: Option<Secret>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,

    /// GitHub Actions run ID (defaults to latest successful `devel` run)
    #[clap(long)]
    pub run_id: Option<u64>,
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
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner authentication token
    #[clap(long)]
    pub token: Option<Secret>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,

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
    pub key: Option<Utf8PathBuf>,

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
    pub key: Option<Utf8PathBuf>,

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
