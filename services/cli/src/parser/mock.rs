use bencher_json::NameId;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliMock {
    /// Number of mock benchmarks to generate
    #[clap(long)]
    pub count: Option<usize>,

    /// Measures to generate for each benchmark
    #[clap(long, default_value = "latency")]
    pub measure: Vec<NameId>,

    /// The power of 10 to use for the mock metrics
    #[clap(long)]
    pub pow: Option<i32>,

    /// Fail while running
    #[clap(long)]
    pub fail: bool,

    /// Intermittently fail while running
    #[clap(long, conflicts_with = "fail")]
    pub flaky: bool,
}
