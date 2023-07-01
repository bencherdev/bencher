use bencher_json::ResourceId;
use clap::{Parser, ValueEnum};
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

    /// Start time (seconds since epoch)
    #[clap(long)]
    pub start_time: Option<i64>,

    /// End time (seconds since epoch)
    #[clap(long)]
    pub end_time: Option<i64>,

    /// Output results in a table
    #[clap(long)]
    pub table: Option<Option<CliPerfTableStyle>>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Supported Table Formats
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliPerfTableStyle {
    /// No styling options
    Empty,
    /// Analog of `empty` but with a vertical space (` `) line
    Blank,
    /// Style which relays only on ASCII charset
    Ascii,
    /// Analog of `ascii` but with rounded corners and without horizontal lines
    AsciiRounded,
    /// Analog of `ascii` which uses UTF-8 charset
    Modern,
    /// Analog of `modern` but without horizontal lines except a header
    Sharp,
    /// Analog of `sharp` but with rounded corners
    Rounded,
    /// Mimics a PostgreSQL table style
    Psql,
    /// Mimics a Markdown table style
    Markdown,
    /// Mimics a ReStructuredText table style
    ReStructuredText,
    /// Style using chars which resembles 2 lines
    Extended,
    /// Style using only ‘.’ and ‘:’ chars with vertical and horizontal split lines
    Dots,
}
