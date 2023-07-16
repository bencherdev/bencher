use bencher_json::{BenchmarkName, ResourceId, Slug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliBenchmark {
    /// List benchmarks
    #[clap(alias = "ls")]
    List(CliBenchmarkList),
    /// Create a benchmark
    #[clap(alias = "add")]
    Create(CliBenchmarkCreate),
    /// View a benchmark
    #[clap(alias = "cat")]
    View(CliBenchmarkView),
    // Update a benchmark
    #[clap(alias = "edit")]
    Update(CliBenchmarkUpdate),
    /// Delete a benchmark
    #[clap(alias = "rm")]
    Delete(CliBenchmarkDelete),
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark name
    #[clap(long)]
    pub name: Option<BenchmarkName>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliBenchmarksSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliBenchmarksSort {
    /// Name of the benchmark
    Name,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkCreate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark name
    pub name: BenchmarkName,

    /// Benchmark slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark UUID
    pub benchmark: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkUpdate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark slug or UUID
    pub benchmark: ResourceId,

    /// Benchmark name
    #[clap(long)]
    pub name: Option<BenchmarkName>,

    /// Benchmark slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkDelete {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Benchmark slug or UUID
    pub benchmark: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
