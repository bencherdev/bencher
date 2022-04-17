use clap::Args;
use clap::Parser;
use clap::Subcommand;

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliBencher {
    /// Shell command path
    #[clap(short, long)]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
    pub flag: Option<String>,

    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,

    /// Benchmark output adapter
    #[clap(short, long, default_value = "rust")]
    pub adapter: String,

    /// Repo subcommand
    #[clap(subcommand)]
    pub backend: Option<CliBackend>,
}

/// Time Series Benchmarking
#[derive(Subcommand, Debug)]
pub enum CliBackend {
    /// Git repo to store data
    Repo(CliRepo),
}

#[derive(Args, Debug)]
pub struct CliRepo {
    /// Git repo url
    #[clap(short, long)]
    pub url: String,

    /// Git repo key
    #[clap(short, long)]
    pub key: Option<String>,

    /// Git branch
    #[clap(short, long)]
    pub branch: Option<String>,

    /// Git commit signature name
    #[clap(short, long)]
    pub name: Option<String>,

    /// Git commit signature email
    #[clap(short, long)]
    pub email: Option<String>,

    /// Git commit message
    #[clap(short, long)]
    pub message: Option<String>,
}
