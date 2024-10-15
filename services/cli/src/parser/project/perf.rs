use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, HeadUuid, MeasureUuid, ResourceId, TestbedUuid,
};
use clap::{Parser, ValueEnum};

use crate::parser::{CliBackend, ElidedOption};

#[derive(Parser, Debug)]
#[allow(clippy::option_option)]
pub struct CliPerf {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch UUIDs
    #[clap(long, required = true, value_name = "BRANCH")]
    pub branches: Vec<BranchUuid>,

    /// Optional branch head UUIDs in the same order as `branches`.
    /// Use an underscore (`_`) to specify the current branch head.
    #[clap(long, required = false, value_name = "HEAD")]
    pub heads: Vec<ElidedOption<HeadUuid>>,

    /// Testbed UUIDs
    #[clap(long, required = true, value_name = "TESTBED")]
    pub testbeds: Vec<TestbedUuid>,

    /// Benchmark UUIDs
    #[clap(long, required = true, value_name = "BENCHMARK")]
    pub benchmarks: Vec<BenchmarkUuid>,

    /// Measure UUIDs
    #[clap(long, required = true, value_name = "MEASURE")]
    pub measures: Vec<MeasureUuid>,

    /// Start time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
    pub start_time: Option<DateTime>,

    /// End time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
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
    /// Mimics a `PostgreSQL` table style
    Psql,
    /// Mimics a Markdown table style
    Markdown,
    /// Mimics a `ReStructuredText` table style
    ReStructuredText,
    /// Style using chars which resembles 2 lines
    Extended,
    /// Style using only ‘.’ and ‘:’ chars with vertical and horizontal split lines
    Dots,
}
