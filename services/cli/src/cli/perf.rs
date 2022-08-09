use chrono::{
    DateTime,
    Utc,
};
use clap::{
    Parser,
    ValueEnum,
};
use uuid::Uuid;

use super::CliBackend;

#[derive(Parser, Debug)]
pub struct CliPerf {
    /// Branch UUIDs
    #[clap(long)]
    pub branches: Vec<Uuid>,

    /// Testbed UUIDs
    #[clap(long)]
    pub testbeds: Vec<Uuid>,

    /// Benchmark UUIDs
    #[clap(long)]
    pub benchmarks: Vec<Uuid>,

    /// Benchmark kind
    #[clap(value_enum, long)]
    pub kind: CliPerfKind,

    /// Start time
    pub start_time: Option<DateTime<Utc>>,

    /// End time
    pub end_time: Option<DateTime<Utc>>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Supported kinds of benchmarks
#[derive(ValueEnum, Debug, Clone)]
pub enum CliPerfKind {
    Latency,
    Throughput,
    Compute,
    Memory,
    Storage,
}
