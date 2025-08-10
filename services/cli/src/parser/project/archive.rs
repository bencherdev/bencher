use bencher_json::{
    BenchmarkNameId, BranchNameId, MeasureNameId, ProjectResourceId, TestbedNameId,
};
use clap::{ArgGroup, Args, Parser};

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliArchive {
    /// Project slug or UUID
    #[clap(long, env = "BENCHER_PROJECT")]
    pub project: ProjectResourceId,

    #[clap(flatten)]
    pub dimension: CliArchiveDimension,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("archive_dimension")
        .required(true)
        .multiple(false)
        .args(&["branch", "testbed", "benchmark", "measure"]),
))]
pub struct CliArchiveDimension {
    /// Branch name, slug, or UUID.
    #[clap(long)]
    pub branch: Option<BranchNameId>,

    /// Testbed name, slug, or UUID.
    #[clap(long)]
    pub testbed: Option<TestbedNameId>,

    /// Benchmark name, slug, or UUID.
    #[clap(long)]
    pub benchmark: Option<BenchmarkNameId>,

    /// Measure name, slug, or UUID.
    #[clap(long)]
    pub measure: Option<MeasureNameId>,
}
