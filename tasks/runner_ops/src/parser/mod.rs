pub mod server;

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
    /// Server name from servers.json
    #[clap(long)]
    pub name: Option<String>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "name")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "name")]
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
    /// Server name from servers.json
    #[clap(long)]
    pub name: Option<String>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "name")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "name")]
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner UUID or slug
    #[clap(long, required_unless_present = "name")]
    pub runner: Option<String>,

    /// Runner authentication token
    #[clap(long, required_unless_present = "name")]
    pub token: Option<String>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,

    /// GitHub Actions run ID (defaults to latest successful `devel` run)
    #[clap(long)]
    pub run_id: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct TaskStart {
    /// Server name from servers.json
    #[clap(long)]
    pub name: Option<String>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "name")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "name")]
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,

    /// Runner UUID or slug
    #[clap(long, required_unless_present = "name")]
    pub runner: Option<String>,

    /// Runner authentication token
    #[clap(long, required_unless_present = "name")]
    pub token: Option<String>,

    /// Bencher API host URL
    #[clap(long)]
    pub host: Option<url::Url>,
}

#[derive(Parser, Debug)]
pub struct TaskStop {
    /// Server name from servers.json
    #[clap(long)]
    pub name: Option<String>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "name")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "name")]
    pub key: Option<Utf8PathBuf>,

    /// SSH user
    #[clap(long)]
    pub user: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TaskLogs {
    /// Server name from servers.json
    #[clap(long)]
    pub name: Option<String>,

    /// IP address or hostname of the server
    #[clap(long, required_unless_present = "name")]
    pub server: Option<String>,

    /// Path to SSH private key
    #[clap(long, required_unless_present = "name")]
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
