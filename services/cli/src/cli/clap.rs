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

    /// User email
    #[clap(short, long)]
    pub email: String,

    /// Benchmark project
    #[clap(short, long)]
    pub project: Option<String>,

    /// Benchmark testbed
    #[clap(flatten)]
    pub testbed: CliTestbed,

    /// Custom backend URL
    #[clap(short, long)]
    pub url: Option<String>,
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
    #[clap(long)]
    pub testbed: Option<String>,

    /// Testbed OS
    #[clap(long, requires = "testbed")]
    pub os: Option<String>,

    /// Testbed OS Version
    #[clap(long, requires = "testbed")]
    pub os_version: Option<String>,

    /// Testbed CPU
    #[clap(long, requires = "testbed")]
    pub cpu: Option<String>,

    /// Testbed RAM
    #[clap(long, requires = "testbed")]
    pub ram: Option<String>,

    /// Testbed Disk Size
    #[clap(long, requires = "testbed")]
    pub disk: Option<String>,

    /// Testbed Architecture
    #[clap(long, requires = "testbed")]
    pub arch: Option<String>,
}
