#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, HeadUuid, MeasureUuid, ProjectResourceId, TestbedUuid,
};
use clap::Parser;

use crate::parser::{CliBackend, ElidedOption};

#[derive(Parser, Debug)]
pub struct CliPerf {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Branch UUIDs
    #[clap(long, required = true, value_name = "BRANCH")]
    pub branches: Vec<BranchUuid>,

    /// Optional branch head UUIDs in the same order as `branches`.
    /// Use an underscore (`_`) to specify the current branch head.
    #[clap(long, required = false, value_name = "HEAD")]
    pub heads: Vec<ElidedOption<HeadUuid>>,

    /// Testbed UUIDs
    #[clap(long, required = true, value_name = "TESTBED")]
    pub testbeds: Vec<TestbedUuid>,

    /// Optional testbed spec UUIDs in the same order as `testbeds`.
    /// Use an underscore (`_`) to specify the current testbed spec.
    #[cfg(feature = "plus")]
    #[clap(long, required = false, value_name = "SPEC")]
    pub specs: Vec<ElidedOption<SpecUuid>>,

    /// Benchmark UUIDs
    #[clap(long, required = true, value_name = "BENCHMARK")]
    pub benchmarks: Vec<BenchmarkUuid>,

    /// Measure UUIDs
    #[clap(long, required = true, value_name = "MEASURE")]
    pub measures: Vec<MeasureUuid>,

    /// Start time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
    pub start_time: Option<DateTime>,

    /// End time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
    pub end_time: Option<DateTime>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[cfg(test)]
mod tests {
    use clap::{Parser as _, error::ErrorKind};

    use crate::parser::CliBencher;

    const UUID: &str = "00000000-0000-4000-8000-000000000001";
    const PERF: [&str; 11] = [
        "bencher",
        "perf",
        "my-project",
        "--branches",
        UUID,
        "--testbeds",
        UUID,
        "--benchmarks",
        UUID,
        "--measures",
        UUID,
    ];

    // `bencher perf` prints JSON only, so a script that asks for a table is refused.
    #[test]
    fn perf_table_is_an_unknown_argument() {
        CliBencher::try_parse_from(PERF).unwrap();
        let err = CliBencher::try_parse_from(PERF.into_iter().chain(["--table"])).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{err}");
    }
}
