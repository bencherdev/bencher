use clap::Args;
use clap::Parser;
use clap::Subcommand;

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliBencher {
    /// Benchmark command
    #[clap(flatten)]
    pub benchmark: CliBenchmark,

    /// Benchmark output adapter
    #[clap(short, long, default_value = "rust")]
    pub adapter: String,

    /// Repo subcommand
    #[clap(subcommand)]
    pub backend: Option<CliBackend>,

    /// Benchmark results output location
    #[clap(flatten)]
    pub output: CliOutput,
}

#[derive(Args, Debug)]
pub struct CliBenchmark {
    /// Shell command path
    #[clap(short, long)]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
    pub flag: Option<String>,

    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,
}

/// Backend data stores
#[derive(Subcommand, Debug)]
pub enum CliBackend {
    /// Git repo backend
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

#[derive(Args, Debug)]
pub struct CliOutput {
    /// Headless output
    #[clap(short, long, group = "output")]
    pub headless: bool,

    /// Default: Bencher website (bencher.dev)
    #[clap(short, long, group = "output")]
    pub web: bool,

    /// Custom output URL
    #[clap(short, long, group = "output")]
    pub url: Option<String>,
}
