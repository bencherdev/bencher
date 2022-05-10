use clap::ArgGroup;
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

    /// Benchmark results output location
    #[clap(flatten)]
    pub new_backend: NewCliBackend,

    /// Backend subcommand
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

    #[clap(flatten)]
    pub push: CliPush,
}

#[derive(Args, Debug)]
pub struct CliPush {
    /// Git add, commit, and push updates
    #[clap(short, long)]
    pub push: bool,

    /// Git commit signature name
    #[clap(short, long, requires = "push")]
    pub name: Option<String>,

    /// Git commit signature email
    #[clap(short, long, requires = "push")]
    pub email: Option<String>,

    /// Git commit message
    #[clap(short, long, requires = "push")]
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

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("backend")
        .required(true)
        .args(&["url", "cloud"]),
))]
pub struct NewCliBackend {
    /// Headless output
    #[clap(short, long)]
    pub url: Option<String>,

    /// Default: Bencher cloud (bencher.dev)
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
