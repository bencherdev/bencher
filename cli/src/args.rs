use clap::Args;
use clap::Parser;
use clap::Subcommand;

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliArgs {
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

    /// Git repo to store data
    #[clap(short, long)]
    pub git: Option<String>,

    /// Git repo key
    #[clap(short, long)]
    pub key: Option<String>,

    /// Git commit signature name
    #[clap(short, long)]
    pub name: Option<String>,

    /// Git commit signature email
    #[clap(short, long)]
    pub email: Option<String>,

    /// Git commit message
    #[clap(short, long)]
    pub message: Option<String>,

    /// Output tags
    #[clap(short, long)]
    pub tag: Option<Vec<String>>,

    /// Repo subcommand
    #[clap(subcommand)]
    pub subcommand: Option<CliSub>,
}

/// Time Series Benchmarking
#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Git repo
    Repo(CliRepo),
}

#[derive(Args, Debug)]
pub struct CliRepo {
    #[clap(short, long)]
    name: Option<String>,
}
