use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
};
use uuid::Uuid;

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliBenchmark {
    /// List benchmarks
    #[clap(alias = "ls")]
    List(CliBenchmarkList),
    /// View a benchmark
    View(CliBenchmarkView),
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark UUID
    pub benchmark: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
