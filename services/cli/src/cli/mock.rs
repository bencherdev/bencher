use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliMock {
    /// Number of mock benchmarks to generate
    #[clap(long)]
    pub count: Option<usize>,

    /// Fail while running
    #[clap(long)]
    pub fail: bool,

    /// Intermittently fail while running
    #[clap(long, conflicts_with = "fail")]
    pub flaky: bool,
}
