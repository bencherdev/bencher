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
    /// IP address or hostname of the server
    #[clap(long)]
    pub server: String,

    /// Path to SSH private key
    #[clap(long)]
    pub key: Utf8PathBuf,

    /// SSH user (default: root)
    #[clap(long, default_value = "root")]
    pub user: String,

    /// Path to runner binary to deploy (Linux `x86_64`)
    #[clap(long)]
    pub runner_binary: Option<Utf8PathBuf>,
}

#[derive(Parser, Debug)]
pub struct TaskDeploy {
    /// IP address or hostname of the server
    #[clap(long)]
    pub server: String,

    /// Path to SSH private key
    #[clap(long)]
    pub key: Utf8PathBuf,

    /// SSH user (default: root)
    #[clap(long, default_value = "root")]
    pub user: String,

    /// Runner UUID or slug
    #[clap(long)]
    pub runner: String,

    /// Runner authentication token
    #[clap(long)]
    pub token: String,

    /// Bencher API host URL
    #[clap(long, default_value = "https://api.bencher.dev")]
    pub host: url::Url,

    /// GitHub Actions run ID (defaults to latest successful `cloud` run)
    #[clap(long)]
    pub run_id: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct TaskStart {
    /// IP address or hostname of the server
    #[clap(long)]
    pub server: String,

    /// Path to SSH private key
    #[clap(long)]
    pub key: Utf8PathBuf,

    /// SSH user (default: root)
    #[clap(long, default_value = "root")]
    pub user: String,

    /// Runner UUID or slug
    #[clap(long)]
    pub runner: String,

    /// Runner authentication token
    #[clap(long)]
    pub token: String,

    /// Bencher API host URL
    #[clap(long, default_value = "https://api.bencher.dev")]
    pub host: url::Url,
}

#[derive(Parser, Debug)]
pub struct TaskStop {
    /// IP address or hostname of the server
    #[clap(long)]
    pub server: String,

    /// Path to SSH private key
    #[clap(long)]
    pub key: Utf8PathBuf,

    /// SSH user (default: root)
    #[clap(long, default_value = "root")]
    pub user: String,
}

#[derive(Parser, Debug)]
pub struct TaskLogs {
    /// IP address or hostname of the server
    #[clap(long)]
    pub server: String,

    /// Path to SSH private key
    #[clap(long)]
    pub key: Utf8PathBuf,

    /// SSH user (default: root)
    #[clap(long, default_value = "root")]
    pub user: String,

    /// Number of lines to show (omit to show all logs)
    #[clap(long)]
    pub lines: Option<u32>,

    /// Follow logs in real-time
    #[clap(long)]
    pub follow: bool,
}
