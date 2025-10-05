use bencher_json::{
    Boundary, BranchNameId, DateTime, GitHash, MeasureNameId, ProjectResourceId, ReportUuid,
    SampleSize, TestbedNameId, Window,
};
use clap::{Args, Parser, Subcommand, ValueEnum};

use super::{branch::CliStartPointUpdate, threshold::CliModelTest};
use crate::parser::{CliBackend, CliPagination, ElidedOption};

#[derive(Subcommand, Debug)]
pub enum CliReport {
    /// List reports
    #[clap(alias = "ls")]
    List(CliReportList),
    /// Create a report
    #[clap(alias = "add")]
    Create(Box<CliReportCreate>),
    /// View a report
    #[clap(alias = "get")]
    View(CliReportView),
    /// Delete a report
    #[clap(alias = "rm")]
    Delete(CliReportDelete),
}

#[derive(Parser, Debug)]
pub struct CliReportList {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Branch name, slug, or UUID
    #[clap(long)]
    pub branch: Option<BranchNameId>,

    /// Testbed name, slug, or UUID
    #[clap(long)]
    pub testbed: Option<TestbedNameId>,

    /// Start time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
    pub start_time: Option<DateTime>,

    /// End time (seconds since epoch)
    #[clap(long, value_name = "SECONDS")]
    pub end_time: Option<DateTime>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliReportsSort>,

    /// Filter for reports with an archived branch or testbed
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliReportsSort {
    /// Date time of the report
    DateTime,
}

#[derive(Parser, Debug)]
pub struct CliReportCreate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Branch name, slug, or UUID
    #[clap(long)]
    pub branch: BranchNameId,

    /// `git` commit hash
    #[clap(long)]
    pub hash: Option<GitHash>,

    #[clap(flatten)]
    pub start_point: CliStartPointUpdate,

    /// Testbed name, slug, or UUID
    #[clap(long)]
    pub testbed: TestbedNameId,

    #[clap(flatten)]
    pub thresholds: CliReportThresholds,

    /// Start time (ISO 8601 formatted string)
    #[clap(long)]
    pub start_time: chrono::DateTime<chrono::Utc>,

    /// End time (ISO 8601 formatted string)
    #[clap(long)]
    pub end_time: chrono::DateTime<chrono::Utc>,

    /// Benchmark results
    #[clap(long)]
    pub results: Vec<String>,

    /// Benchmark harness adapter
    #[clap(value_enum, long)]
    pub adapter: Option<CliReportAdapter>,

    /// Benchmark harness suggested central tendency (ie average)
    #[clap(value_enum, long)]
    pub average: Option<CliReportAverage>,

    /// Fold multiple results into a single result
    #[clap(value_enum, long)]
    pub fold: Option<CliReportFold>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
pub struct CliReportThresholds {
    /// Threshold Measure name, slug, or UUID
    /// When specifying multiple Thresholds, all of the same options must be used for each Threshold.
    /// To ignore an option for a specific Threshold, use an underscore (`_`).
    #[clap(long)]
    pub threshold_measure: Vec<MeasureNameId>,

    /// Threshold model test
    #[clap(value_enum, long, requires = "threshold_measure")]
    pub threshold_test: Vec<CliModelTest>,

    /// Minimum sample size
    /// To ignore a this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_min_sample_size: Vec<ElidedOption<SampleSize>>,

    /// Maximum sample size
    /// To ignore a this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_max_sample_size: Vec<ElidedOption<SampleSize>>,

    /// Window size (seconds)
    /// To ignore a this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_window: Vec<ElidedOption<Window>>,

    /// Lower boundary
    /// To ignore a this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_lower_boundary: Vec<ElidedOption<Boundary>>,

    /// Upper boundary
    /// To ignore a this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_upper_boundary: Vec<ElidedOption<Boundary>>,

    /// Reset all unspecified Thresholds for the `branch` and `testbed`
    /// If a Threshold already exists and is not specified, its current Model will be removed.
    #[clap(long)]
    pub thresholds_reset: bool,
}

/// Supported Adapters
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliReportAdapter {
    /// 🪄 Magic
    Magic,
    /// {...} JSON
    Json,
    // TODO remove in due time
    #[clap(hide = true)]
    CSharp,
    /// #️⃣ C# `DotNet`
    CSharpDotNet,
    // TODO remove in due time
    #[clap(hide = true)]
    Cpp,
    /// ➕ C++ Catch2
    CppCatch2,
    /// ➕ C++ Google
    CppGoogle,
    // TODO remove in due time
    #[clap(hide = true)]
    Go,
    /// 🕳 Go Bench
    GoBench,
    // TODO remove in due time
    #[clap(hide = true)]
    Java,
    /// ☕️ Java JMH
    JavaJmh,
    // TODO remove in due time
    #[clap(hide = true)]
    Js,
    /// 🕸 JavaScript Benchmark
    JsBenchmark,
    /// 🕸 JavaScript Time
    JsTime,
    // TODO remove in due time
    #[clap(hide = true)]
    Python,
    /// 🐍 Python ASV
    PythonAsv,
    /// 🐍 Python Pytest
    PythonPytest,
    // TODO remove in due time
    #[clap(hide = true)]
    Ruby,
    /// ♦️ Ruby Benchmark
    RubyBenchmark,
    // TODO remove in due time
    #[clap(hide = true)]
    Rust,
    /// 🦀 Rust Bench
    RustBench,
    /// 🦀 Rust Criterion
    RustCriterion,
    /// 🦀 Rust Iai
    RustIai,
    /// 🦀 Rust Gungraun
    // Iai-Callgrind was renamed to Gungraun
    // https://github.com/bencherdev/bencher/issues/619
    #[clap(alias = "rust_iai_callgrind")]
    RustGungraun,
    // TODO remove in due time
    #[clap(hide = true)]
    Shell,
    /// ❯_ Shell Hyperfine
    ShellHyperfine,
}

/// Suggested Central Tendency (Average)
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliReportAverage {
    /// Mean and standard deviation
    Mean,
    /// Median and interquartile range
    Median,
}

/// Supported Fold Operations
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliReportFold {
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Mean of values
    Mean,
    /// Median of values
    Median,
}

#[derive(Parser, Debug)]
pub struct CliReportView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Report UUID
    pub report: ReportUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliReportDelete {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Report UUID
    pub report: ReportUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
