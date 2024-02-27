use bencher_json::{BenchmarkUuid, BranchUuid, DateTime, MeasureUuid, ResourceId, TestbedUuid};
use clap::{Parser, ValueEnum};

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
#[allow(clippy::option_option)]
pub struct CliPerf {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch UUIDs
    #[clap(long, required = true)]
    pub branches: Vec<BranchUuid>,

    /// Testbed UUIDs
    #[clap(long, required = true)]
    pub testbeds: Vec<TestbedUuid>,

    /// Benchmark UUIDs
    #[clap(long, required = true)]
    pub benchmarks: Vec<BenchmarkUuid>,

    /// Measure UUIDs
    #[clap(long, required = true)]
    pub measures: Vec<MeasureUuid>,

    /// Start time (seconds since epoch)
    #[clap(long)]
    pub start_time: Option<DateTime>,

    /// End time (seconds since epoch)
    #[clap(long)]
    pub end_time: Option<DateTime>,

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
