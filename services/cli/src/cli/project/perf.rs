use bencher_json::ResourceId;
use clap::Parser;
use uuid::Uuid;

use crate::cli::CliBackend;

#[derive(Parser, Debug)]
pub struct CliPerf {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind slug or UUID
    #[clap(long)]
    pub metric_kind: ResourceId,

    /// Branch UUIDs
    #[clap(long)]
    pub branches: Vec<Uuid>,

    /// Testbed UUIDs
    #[clap(long)]
    pub testbeds: Vec<Uuid>,

    /// Benchmark UUIDs
    #[clap(long)]
    pub benchmarks: Vec<Uuid>,

    /// Start time
    #[clap(long)]
    pub start_time: Option<i64>,

    /// End time
    #[clap(long)]
    pub end_time: Option<i64>,

    /// Output results in a table
    #[clap(long)]
    pub table: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}
