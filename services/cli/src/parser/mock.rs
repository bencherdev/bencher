use bencher_json::{BmfVersion, MeasureNameId, ParameterSet};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliMock {
    /// Seed for pseudorandom number generator
    #[clap(long)]
    pub seed: Option<u64>,

    /// Number of mock benchmarks to generate
    #[clap(long)]
    pub count: Option<usize>,

    /// Measures to generate for each benchmark.
    /// At BMF version 1, each measure has one metric, `value`.
    /// At BMF version 0, each measure has a `value`, a `lower_value`, and an `upper_value`.
    #[clap(long, default_value = "latency")]
    pub measure: Vec<MeasureNameId>,

    /// Parameters of a variant to generate for every benchmark, as a JSON object (BMF version 1).
    /// Each set of parameters makes one variant of every benchmark, for example `--parameters '{"size": 64}'`.
    /// If not set, each benchmark has one variant with no parameters.
    #[clap(long)]
    pub parameters: Vec<ParameterSet>,

    /// BMF version of the mock results.
    /// A project at BMF version 0 needs `--bmf-version 0`.
    #[clap(long, default_value_t = BmfVersion::V1)]
    pub bmf_version: BmfVersion,

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
