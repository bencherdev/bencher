use bencher_json::{
    Boundary, BranchNameId, DateTime, GitHash, MeasureNameId, MetricName, ParameterSet,
    ProjectResourceId, ReportUuid, SampleSize, TestbedNameId, Window,
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

    /// Include the full results and alerts for each report
    #[clap(long)]
    pub expand: bool,

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

    /// Benchmark results: harness output, or BMF for the `json` adapters.
    ///
    /// Read at the project's BMF version, or at the version `json_v0` or `json_v1` names.
    #[clap(long)]
    pub results: Vec<String>,

    /// Benchmark harness adapter
    ///
    /// Results are read at the project's BMF version. `json_v0` and `json_v1` read JSON at their own BMF version on any project.
    #[clap(value_enum, long)]
    pub adapter: Option<CliReportAdapter>,

    /// Benchmark harness suggested central tendency (ie average)
    ///
    /// Ignored with a warning at BMF version 1, where every average is its own metric.
    #[clap(value_enum, long)]
    pub average: Option<CliReportAverage>,

    /// Fold multiple results into a single result
    ///
    /// BMF version 0 only; ignored at version 1 with a warning.
    #[clap(value_enum, long)]
    pub fold: Option<CliReportFold>,

    /// Measure name, slug, or UUID to keep from the results, repeatable.
    /// Every other measure is dropped before it is stored. If not set, every measure is kept.
    #[clap(long)]
    pub measure: Vec<MeasureNameId>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
pub struct CliReportThresholds {
    /// Threshold Measure name, slug, or UUID
    /// When specifying multiple Thresholds, all of the same options must be used for each Threshold.
    /// To ignore an option for a specific Threshold, use an underscore (`_`).
    ///
    /// At BMF version 1, name each Threshold's metric with `--threshold-metric`. Thresholds without a metric are skipped with a warning.
    #[clap(long)]
    pub threshold_measure: Vec<MeasureNameId>,

    /// Threshold metric name (BMF version 1)
    ///
    /// Once any Threshold has a metric or parameters, every Threshold needs a metric.
    /// Refused at BMF version 0.
    #[clap(long, requires = "threshold_test")]
    pub threshold_metric: Vec<ElidedOption<MetricName>>,

    /// Threshold parameters filter, as a JSON object of the parameters a variant must have (BMF version 1)
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    ///
    /// Refused at BMF version 0.
    #[clap(long, requires = "threshold_test")]
    pub threshold_parameters: Vec<ElidedOption<ParameterSet>>,

    /// Threshold model test
    #[clap(value_enum, long, requires = "threshold_measure")]
    pub threshold_test: Vec<CliModelTest>,

    /// Minimum historical sample size
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_min_sample_size: Vec<ElidedOption<SampleSize>>,

    /// Maximum historical sample size
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_max_sample_size: Vec<ElidedOption<SampleSize>>,

    /// Window size (seconds)
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_window: Vec<ElidedOption<Window>>,

    /// Lower boundary
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_lower_boundary: Vec<ElidedOption<Boundary>>,

    /// Upper boundary
    /// To ignore this option when specifying multiple Thresholds, use an underscore (`_`).
    #[clap(long, requires = "threshold_test")]
    pub threshold_upper_boundary: Vec<ElidedOption<Boundary>>,

    /// Reset the unspecified Thresholds of the `branch` and `testbed`
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
    /// {...} JSON at BMF version 0, on any project
    JsonV0,
    /// {...} JSON at BMF version 1, on any project
    JsonV1,
    /// #️⃣ C# `DotNet`
    CSharpDotNet,
    /// ➕ C++ Catch2
    CppCatch2,
    /// ➕ C++ Google
    CppGoogle,
    /// 🎯 Dart `benchmark_harness`
    DartBenchmarkHarness,
    /// 🕳 Go Bench
    GoBench,
    /// ☕️ Java JMH
    JavaJmh,
    /// 🕸 JavaScript Benchmark
    JsBenchmark,
    /// 🕸 JavaScript Deno
    JsDeno,
    /// 🕸 JavaScript mitata
    JsMitata,
    /// 🕸 JavaScript Time
    JsTime,
    /// 🕸 JavaScript tinybench
    JsTinybench,
    /// 🕸 JavaScript Vitest
    JsVitest,
    /// 🐍 Python ASV
    PythonAsv,
    /// 🐍 Python Pytest
    PythonPytest,
    /// ♦️ Ruby Benchmark
    RubyBenchmark,
    /// ♦️ Ruby benchmark-ips
    RubyBenchmarkIps,
    /// 🦀 Rust Bench
    RustBench,
    /// 🦀 Rust Criterion
    RustCriterion,
    /// 🦀 Rust Divan
    RustDivan,
    /// 🦀 Rust Iai
    RustIai,
    /// 🦀 Rust Gungraun
    // Iai-Callgrind was renamed to Gungraun
    // https://github.com/bencherdev/bencher/issues/619
    #[clap(alias = "rust_iai_callgrind")]
    RustGungraun,
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
