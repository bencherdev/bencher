use bencher_json::{BenchmarkName, BenchmarkResourceId, BenchmarkSlug, ProjectResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliArchived, CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliBenchmark {
    /// List benchmarks
    #[clap(alias = "ls")]
    List(CliBenchmarkList),
    /// Create a benchmark
    #[clap(alias = "add")]
    Create(CliBenchmarkCreate),
    /// View a benchmark
    #[clap(alias = "get")]
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
    pub project: ProjectResourceId,

    /// Benchmark name
    #[clap(long)]
    pub name: Option<BenchmarkName>,

    /// Benchmark search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliBenchmarksSort>,

    /// Filter for archived benchmarks
    #[clap(long)]
    pub archived: bool,

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
    pub project: ProjectResourceId,

    /// Benchmark name
    #[clap(long)]
    pub name: BenchmarkName,

    /// Benchmark slug
    #[clap(long)]
    pub slug: Option<BenchmarkSlug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Benchmark slug or UUID
    pub benchmark: BenchmarkResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkUpdate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Benchmark slug or UUID
    pub benchmark: BenchmarkResourceId,

    /// Benchmark name
    #[clap(long)]
    pub name: Option<BenchmarkName>,

    /// Benchmark slug
    #[clap(long)]
    pub slug: Option<BenchmarkSlug>,

    #[clap(flatten)]
    pub archived: CliArchived,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBenchmarkDelete {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Benchmark slug or UUID
    pub benchmark: BenchmarkResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
