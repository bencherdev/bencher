//! What the console reads to draw its pages.
//!
//! These shapes are unpublished: they follow the console and change with it.
//! Dimensions and threshold models appear once per response in tables, and lines
//! refer to them by index. Points are columns aligned to one shared x.

use bencher_valid::{
    BenchmarkName, Boundary, BranchName, DateTimeMillis, GitHash, MetricName, ModelTest,
    ResourceName, SampleSize, Window,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AlertUuid, BenchmarkSlug, BenchmarkUuid, BranchSlug, BranchUuid, HeadUuid, MeasureSlug,
    MeasureUuid, ModelUuid, ParameterSet, ReportUuid, TestbedSlug, TestbedUuid, ThresholdUuid,
    VariantUuid,
};

use super::{
    alert::AlertStatus,
    boundary::BoundaryLimit,
    head::{JsonVersion, VersionNumber},
    report::{Adapter, Iteration, JsonReportAlertsCounts},
};

/// How many lines a report page returns when the request does not say.
pub const DEFAULT_CONSOLE_LINES_PER_PAGE: u8 = 32;

/// The most reports a report page's history reaches back over: the newest are
/// kept, and the window's start moves up to the oldest of them.
pub const MAX_CONSOLE_HISTORY_REPORTS: usize = 256;

/// The longest window a report page asks for in days.
pub const MAX_CONSOLE_WINDOW_DAYS: u16 = 366;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleReportQueryParams {
    /// The start of the history window, in milliseconds.
    /// The window ends with the report, and a start after the report is the report's.
    /// Defaults to four weeks before the report.
    pub start_time: Option<DateTimeMillis>,
    /// The history window in whole days before the report's start, from 1 to 366,
    /// instead of a `start_time`.
    pub window: Option<u16>,
    /// Only the lines whose benchmark name, variant parameters, measure name, or
    /// metric name contain this text, ignoring case.
    pub search: Option<String>,
    /// Only the lines of this measure.
    pub measure: Option<MeasureUuid>,
    /// Only the lines of this metric name.
    pub metric: Option<MetricName>,
    /// Only the lines whose variant carries every key and value of this JSON
    /// parameter set.
    pub parameters: Option<String>,
    /// How to group the lines.
    pub group: Option<ConsoleLineGroup>,
    /// How to order the lines inside each group.
    pub sort: Option<ConsoleLineSort>,
    /// The page of lines to return, starting at 1.
    pub page: Option<u32>,
    /// The number of lines per page. Zero returns no lines, only the report and
    /// its counts.
    pub per_page: Option<u8>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLineGroup {
    #[default]
    Benchmark,
    Measure,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConsoleLineSort {
    /// Groups with an alert first, and inside a group alerting lines first,
    /// then by benchmark, variant parameters, measure, and metric name.
    #[default]
    Name,
    /// Groups and lines by their delta toward the side their threshold guards,
    /// worst first, with lines that no threshold checked last.
    Delta,
}

/// One report as the console's report page draws it: its identity, and one page
/// of its lines in drawing order, each with its history over the window that ends
/// at the report.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleReport {
    pub uuid: ReportUuid,
    pub branch: JsonConsoleBranch,
    pub testbed: JsonConsoleTestbed,
    pub version: JsonVersion,
    pub start_time: DateTimeMillis,
    pub end_time: DateTimeMillis,
    pub adapter: Adapter,
    pub counts: JsonConsoleReportCounts,
    /// The report before this one on the same branch and testbed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<JsonConsoleReportLink>,
    /// The report after this one on the same branch and testbed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<JsonConsoleReportLink>,
    /// The history window, which ends at this report.
    pub window: JsonConsoleWindow,
    /// The number of lines in the report that match the search and filters.
    pub total: u32,
    /// Every group of those lines in drawing order, on the first page only.
    pub groups: Vec<JsonConsoleLineGroup>,
    /// The page of lines, in drawing order.
    pub lines: Vec<JsonConsoleReportLine>,
    pub points: JsonConsolePoints,
    pub reports: Vec<JsonConsolePointReport>,
    pub benchmarks: Vec<JsonConsoleBenchmark>,
    pub variants: Vec<JsonConsoleVariant>,
    pub measures: Vec<JsonConsoleMeasure>,
    pub models: Vec<JsonConsoleModel>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleReportCounts {
    pub benchmarks: u32,
    pub variants: u32,
    pub measures: u32,
    /// The distinct metric names of the report's lines.
    pub metrics: u32,
    pub alerts: JsonReportAlertsCounts,
}

/// A neighboring report, with what its Previous or Next control shows.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleReportLink {
    pub uuid: ReportUuid,
    pub start_time: DateTimeMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<GitHash>,
    pub adapter: Adapter,
}

/// The window a response read, after any clamp.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleWindow {
    pub start_time: DateTimeMillis,
    pub end_time: DateTimeMillis,
    /// Whether the requested start was moved later, because a history holds a
    /// bounded number of reports.
    pub clamped: bool,
}

/// A group of lines and its totals, which hold whichever page is loaded.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleLineGroup {
    /// The group's benchmark or measure, an index into `benchmarks` or `measures`.
    pub key: u32,
    pub lines: u32,
    pub variants: u32,
    /// The number of its lines that alerted.
    pub alerts: u32,
}

/// One line of a report: one metric name of one measure of one variant.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleReportLine {
    /// An index into `benchmarks`.
    pub benchmark: u32,
    /// An index into `variants`.
    pub variant: u32,
    /// An index into `measures`.
    pub measure: u32,
    pub metric: MetricName,
    /// The value in this report.
    pub value: f64,
    /// The model of the threshold that checked the value, an index into `models`.
    /// Absent when no threshold checked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<JsonConsoleAlert>,
    /// The line over the window, aligned to `points`.
    pub history: JsonConsoleSeries,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlert {
    pub uuid: AlertUuid,
    pub limit: BoundaryLimit,
    pub status: AlertStatus,
}

/// The shared x of every series in a response: one point per report and
/// iteration, in report order.
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsolePoints {
    /// The report start time of each point, in milliseconds.
    pub x: Vec<DateTimeMillis>,
    /// The report of each point, an index into `reports`.
    pub report: Vec<u32>,
    /// The iteration of each point, absent when every point is the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<Vec<Iteration>>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsolePointReport {
    pub uuid: ReportUuid,
    pub version: VersionNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<GitHash>,
}

/// One line's values, aligned to `points`, with null where the line has no point.
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleSeries {
    #[typeshare(typescript(type = "(number | null)[]"))]
    pub y: Vec<Option<f64>>,
    /// The baseline the threshold compared each point with.
    /// Absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "(number | null)[]"))]
    pub baseline: Option<Vec<Option<f64>>>,
    /// Absent when there is no lower limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "(number | null)[]"))]
    pub lower: Option<Vec<Option<f64>>>,
    /// Absent when there is no upper limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "(number | null)[]"))]
    pub upper: Option<Vec<Option<f64>>>,
    /// The points that alerted, in point order.
    pub alerts: Vec<JsonConsoleAlertPoint>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlertPoint {
    /// An index into the points.
    pub index: u32,
    pub uuid: AlertUuid,
    pub limit: BoundaryLimit,
    pub status: AlertStatus,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBranch {
    pub uuid: BranchUuid,
    pub name: BranchName,
    pub slug: BranchSlug,
    /// The head the lines come from: a report's own, or the one a plot read.
    pub head: HeadUuid,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleTestbed {
    pub uuid: TestbedUuid,
    pub name: ResourceName,
    pub slug: TestbedSlug,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBenchmark {
    pub uuid: BenchmarkUuid,
    pub name: BenchmarkName,
    pub slug: BenchmarkSlug,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleVariant {
    pub uuid: VariantUuid,
    /// An index into `benchmarks`.
    pub benchmark: u32,
    pub parameters: ParameterSet,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleMeasure {
    pub uuid: MeasureUuid,
    pub name: ResourceName,
    pub slug: MeasureSlug,
    pub units: ResourceName,
}

/// A threshold model, which the line or series it checked refers to by index.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleModel {
    pub uuid: ModelUuid,
    pub threshold: ThresholdUuid,
    pub test: ModelTest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_boundary: Option<Boundary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_boundary: Option<Boundary>,
}
