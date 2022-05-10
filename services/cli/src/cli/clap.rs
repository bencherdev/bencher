use clap::ArgGroup;
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
#[clap(group(
    ArgGroup::new("backend")
        .required(true)
        .args(&["url", "cloud"]),
))]
pub struct CliBackend {
    /// Custom reporting backend URL
    #[clap(short, long)]
    pub url: String,

    /// Default: Bencher cloud backend (https:bencher.dev)
    #[clap(flatten)]
    pub cloud: CliCloud,
}

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("testbed")
        .required(false)
        .args(&["testbed_id", "testbed"]),
))]
pub struct CliCloud {
    /// User email
    #[clap(short, long)]
    pub email: String,

    /// User project
    #[clap(short, long)]
    pub project: Option<String>,

    /// Testbed ID
    #[clap(short, long)]
    pub testbed_id: Option<String>,

    /// Testbed
    #[clap(flatten)]
    pub testbed: CliTestbed,
}

#[derive(Args, Debug)]
pub struct CliTestbed {
    /// Testbed OS
    #[clap(short, long)]
    pub os: Option<String>,

    /// Testbed OS Version
    #[clap(short, long)]
    pub version: Option<String>,

    /// Testbed CPU
    #[clap(short, long)]
    pub cpu: Option<String>,

    /// Testbed RAM
    #[clap(short, long)]
    pub ram: Option<String>,

    /// Testbed Disk Size
    #[clap(short, long)]
    pub disk: Option<String>,

    /// Testbed Architecture
    #[clap(short, long)]
    pub arch: Option<String>,
}
