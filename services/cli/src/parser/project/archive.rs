use bencher_json::{NameId, ResourceId};
use clap::{ArgGroup, Args, Parser};

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliArchive {
    /// Project slug or UUID
    #[clap(long, env = "BENCHER_PROJECT")]
    pub project: ResourceId,

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
    pub branch: Option<NameId>,

    /// Testbed name, slug, or UUID.
    #[clap(long)]
    pub testbed: Option<NameId>,

    /// Benchmark name, slug, or UUID.
    #[clap(long)]
    pub benchmark: Option<NameId>,

    /// Measure name, slug, or UUID.
    #[clap(long)]
    pub measure: Option<NameId>,
}
