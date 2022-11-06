use bencher_json::ResourceId;
use chrono::{DateTime, Utc};
use clap::Parser;
use uuid::Uuid;

use crate::cli::CliBackend;

#[derive(Parser, Debug)]
pub struct CliPerf {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch UUIDs
    #[clap(long)]
    pub branches: Vec<Uuid>,

    /// Testbed UUIDs
    #[clap(long)]
    pub testbeds: Vec<Uuid>,

    /// Benchmark UUIDs
    #[clap(long)]
    pub benchmarks: Vec<Uuid>,

    /// Benchmark kind slug or UUID
    #[clap(value_enum, long)]
    pub kind: ResourceId,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// End time
    pub end_time: Option<DateTime<Utc>>,

    #[clap(flatten)]
    pub backend: CliBackend,
}
