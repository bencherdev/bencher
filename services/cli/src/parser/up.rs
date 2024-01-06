use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliUp {
    /// Number of mock benchmarks to generate
    #[clap(long)]
    pub count: Option<usize>,

    /// Generate values of 10 raised to this power
    #[clap(long)]
    pub pow: Option<i32>,

    /// Fail while running
    #[clap(long)]
    pub fail: bool,

    /// Intermittently fail while running
    #[clap(long, conflicts_with = "fail")]
    pub flaky: bool,
}
