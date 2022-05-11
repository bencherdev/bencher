use clap::Args;
use clap::Parser;

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

    /// Testbed
    #[clap(flatten)]
    pub testbed: CliTestbed,

    /// Benchmark reporting backend
    #[clap(flatten)]
    pub backend: CliBackend,
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

#[derive(Args, Debug)]
pub struct CliTestbed {
    /// Testbed ID
    #[clap(short, long)]
    pub testbed: Option<String>,

    /// Testbed OS
    #[clap(long)]
    pub os: Option<String>,

    /// Testbed OS Version
    #[clap(long)]
    pub os_version: Option<String>,

    /// Testbed CPU
    #[clap(long)]
    pub cpu: Option<String>,

    /// Testbed RAM
    #[clap(long)]
    pub ram: Option<String>,

    /// Testbed Disk Size
    #[clap(long)]
    pub disk: Option<String>,

    /// Testbed Architecture
    #[clap(long)]
    pub arch: Option<String>,
}

#[derive(Args, Debug)]
pub struct CliBackend {
    /// Custom reporting backend URL
    #[clap(short, long)]
    pub url: Option<String>,

    /// User email
    #[clap(short, long)]
    pub email: String,

    /// User project
    #[clap(short, long)]
    pub project: Option<String>,
}
