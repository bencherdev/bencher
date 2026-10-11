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

#[cfg(feature = "plus")]
use crate::SpecUuid;
use crate::{
    AlertUuid, BenchmarkSlug, BenchmarkUuid, BranchSlug, BranchUuid, HeadUuid, JsonPerfQuery,
    MeasureSlug, MeasureUuid, ModelUuid, ParameterSet, ReportUuid, TestbedSlug, TestbedUuid,
    ThresholdUuid, VariantUuid,
    urlencoded::{UrlEncodedError, from_urlencoded_list},
};

use super::{
    alert::AlertStatus,
    boundary::BoundaryLimit,
    head::{JsonVersion, VersionNumber},
    perf::{JsonPerfQueryParams, MAX_DIMENSION_ENTRIES},
    report::{Adapter, Iteration, JsonReportAlertsCounts},
};

/// How many lines a report page returns when the request does not say.
pub const DEFAULT_CONSOLE_LINES_PER_PAGE: u8 = 32;

/// The most reports a report page's history reaches back over: the newest are
/// kept, and the window's start moves up to the oldest of them.
pub const MAX_CONSOLE_HISTORY_REPORTS: usize = 256;

/// The longest window a report page asks for in days.
pub const MAX_CONSOLE_WINDOW_DAYS: u16 = 366;

/// The size a line's history is thinned to when the request does not say.
pub const DEFAULT_CONSOLE_HISTORY_POINTS: u16 = 64;

/// The largest size a request may ask a line's history to be thinned to.
pub const MAX_CONSOLE_HISTORY_POINTS: u16 = 256;

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
    /// The size to thin each line's history to, from 2 to 256.
    /// A longer history keeps the lowest and highest value of each stretch of it,
    /// plus every alerting point and the report's own point, which can take it past
    /// this size.
    /// Defaults to 64.
    pub points: Option<u16>,
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
    /// Whether the requested start was moved later: a report's history holds a
    /// bounded number of reports, and an unauthenticated plot reaches back at
    /// most three months.
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

/// One line's values, aligned to `points` with null where the line has no
/// point, or, for a thinned line, one per position in `index`.
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleSeries {
    /// The position in `points` of each value, present only when the line was
    /// thinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<Vec<u32>>,
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
    /// A position in the series' columns.
    pub index: u32,
    pub uuid: AlertUuid,
    pub limit: BoundaryLimit,
    pub status: AlertStatus,
}

/// A benchmark's newest report, where Explore starts a query that names only
/// the benchmark.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleLatestReport {
    pub uuid: ReportUuid,
    pub branch: JsonConsoleBranch,
    pub testbed: JsonConsoleTestbed,
    pub version: JsonVersion,
    pub start_time: DateTimeMillis,
    /// The measures the benchmark reported in it, in name order.
    pub measures: Vec<JsonConsoleMeasure>,
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
    /// The spec a plot read this testbed's runs on, when the query named one.
    #[cfg(feature = "plus")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<SpecUuid>,
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

/// How many alerts a page of alerts holds when the request does not say.
pub const DEFAULT_CONSOLE_ALERTS_PER_PAGE: u8 = 32;

/// The most alerts a page of alerts holds.
pub const MAX_CONSOLE_ALERTS_PER_PAGE: u8 = 64;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlertsQueryParams {
    /// The alerts to list by their status now.
    /// Defaults to `active`.
    pub status: Option<ConsoleAlertStatus>,
    /// A comma separated list of the branch UUIDs the alerts were raised on.
    pub branches: Option<String>,
    /// A comma separated list of the testbed UUIDs the alerts were raised on.
    pub testbeds: Option<String>,
    /// A comma separated list of the measure UUIDs the alerts were raised on.
    pub measures: Option<String>,
    /// A comma separated list of the threshold UUIDs that raised the alerts.
    pub thresholds: Option<String>,
    /// A comma separated list of the report UUIDs that raised the alerts.
    pub reports: Option<String>,
    /// The earliest time the report that raised an alert was created, in milliseconds, inclusive.
    pub start_time: Option<DateTimeMillis>,
    /// The latest time the report that raised an alert was created, in milliseconds, inclusive.
    pub end_time: Option<DateTimeMillis>,
    /// Each alert's history window in whole days before its report's start, from 1 to 366.
    /// Defaults to four weeks.
    pub window: Option<u16>,
    /// The size to thin each alert's history to, from 2 to 256, as a report's lines are.
    /// Defaults to 64.
    pub points: Option<u16>,
    /// The page of alerts to return, starting at 1.
    pub page: Option<u32>,
    /// The number of alerts per page, at most 64. Zero returns only the counts.
    pub per_page: Option<u8>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConsoleAlertStatus {
    #[default]
    Active,
    /// Dismissed or silenced.
    Dismissed,
    All,
}

/// A page of a project's alerts, grouped by the report that raised them, newest
/// report first.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlerts {
    /// The number of alerts that match the filters and the status, over every page.
    pub total: u32,
    /// The number of alerts that match the filters, by their status now, whatever
    /// status the request asked for.
    pub counts: JsonConsoleAlertsCounts,
    pub groups: Vec<JsonConsoleAlertGroup>,
    /// The reports of every group's points.
    pub reports: Vec<JsonConsolePointReport>,
    pub branches: Vec<JsonConsoleBranch>,
    pub testbeds: Vec<JsonConsoleTestbed>,
    pub benchmarks: Vec<JsonConsoleBenchmark>,
    pub variants: Vec<JsonConsoleVariant>,
    pub measures: Vec<JsonConsoleMeasure>,
    pub models: Vec<JsonConsoleModel>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlertsCounts {
    pub active: u32,
    pub dismissed: u32,
    pub silenced: u32,
}

/// The alerts on a page that one report raised, each with its history over a
/// window that ends at the report.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlertGroup {
    /// The report that raised the alerts.
    pub uuid: ReportUuid,
    /// An index into `branches`.
    pub branch: u32,
    /// An index into `testbeds`.
    pub testbed: u32,
    pub version: JsonVersion,
    pub start_time: DateTimeMillis,
    pub end_time: DateTimeMillis,
    /// When the API took the report.
    pub created: DateTimeMillis,
    pub adapter: Adapter,
    /// The number of the report's alerts that match the filters and the status,
    /// over every page.
    pub total: u32,
    /// The history window, which ends at the report.
    pub window: JsonConsoleWindow,
    /// The shared x of the group's histories, whose reports index into `reports`.
    pub points: JsonConsolePoints,
    /// The group's alerts on this page, in the order the report's lines are drawn.
    pub alerts: Vec<JsonConsoleAlertLine>,
}

/// An alert as the line it raised on, the way the report draws that line.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleAlertLine {
    /// The line, following the threshold that raised the alert.
    pub line: JsonConsoleReportLine,
    /// When the alert last changed status, or was raised.
    pub modified: DateTimeMillis,
}

/// The most lines a plot draws.
pub const MAX_CONSOLE_PLOT_LINES: usize = 64;

/// The plot query: the perf query's boxes and the metric names to draw.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsolePerfQueryParams {
    /// A comma separated list of branch UUIDs to query.
    /// Only the first 8 branches are queried.
    pub branches: String,
    /// An optional comma separated list of branch head UUIDs.
    /// To not specify a particular branch head leave an empty entry in the list.
    pub heads: Option<String>,
    /// A comma separated list of testbed UUIDs to query.
    /// Only the first 8 testbeds are queried.
    pub testbeds: String,
    /// An optional comma separated list of testbed spec UUIDs.
    /// To not specify a particular testbed spec leave an empty entry in the list.
    pub specs: Option<String>,
    /// A comma separated list of benchmark UUIDs to query.
    /// Only the first 8 benchmarks are queried.
    pub benchmarks: String,
    /// An optional comma separated list of URL encoded parameters to filter on.
    /// A variant is queried when at least one of them is a subset of its parameters.
    /// Only the first 8 are read.
    pub parameters: Option<String>,
    /// A comma separated list of measure UUIDs to query.
    /// Only the first 8 measures are queried.
    pub measures: String,
    /// An optional comma separated list of URL encoded metric names to draw.
    /// Leaving this off draws each branch, testbed, variant, and measure with every
    /// metric name it reported in the window.
    /// Only the first 8 are read.
    pub metrics: Option<String>,
    /// The start of the window in milliseconds.
    /// Defaults to four weeks before its end.
    pub start_time: Option<DateTimeMillis>,
    /// The end of the window in milliseconds.
    /// Defaults to now.
    pub end_time: Option<DateTimeMillis>,
}

/// The validated plot query.
#[derive(Debug, Clone)]
pub struct JsonConsolePerfQuery {
    pub perf: JsonPerfQuery,
    /// `None` draws each branch, testbed, variant, and measure with every metric
    /// name it reported in the window.
    pub metrics: Option<Vec<MetricName>>,
}

impl TryFrom<JsonConsolePerfQueryParams> for JsonConsolePerfQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonConsolePerfQueryParams) -> Result<Self, Self::Error> {
        let JsonConsolePerfQueryParams {
            branches,
            heads,
            testbeds,
            specs,
            benchmarks,
            parameters,
            measures,
            metrics,
            start_time,
            end_time,
        } = query_params;
        let perf = JsonPerfQueryParams {
            branches,
            heads,
            testbeds,
            specs,
            benchmarks,
            parameters,
            measures,
            start_time,
            end_time,
        }
        .try_into()?;
        // An empty string is no list at all, the way the perf query reads its parameters.
        let metrics = if let Some(metrics) = metrics.as_deref()
            && !metrics.is_empty()
        {
            let mut metrics: Vec<MetricName> = from_urlencoded_list(metrics)?;
            metrics.truncate(MAX_DIMENSION_ENTRIES);
            Some(metrics)
        } else {
            None
        };
        Ok(Self { perf, metrics })
    }
}

/// The lines of a plot query, drawn as columns aligned to one shared x.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsolePerf {
    /// The window the query read, after any clamp.
    pub window: JsonConsoleWindow,
    /// The number of lines the query names: the product of its boxes.
    /// More than `lines` holds when the line cap cut some off.
    pub total: u32,
    /// The lines in the order the boxes name them: branch, testbed, benchmark,
    /// variant, measure, and metric. A line with no point in the window has only
    /// nulls.
    pub lines: Vec<JsonConsolePerfLine>,
    pub points: JsonConsolePoints,
    pub reports: Vec<JsonConsolePointReport>,
    pub branches: Vec<JsonConsoleBranch>,
    pub testbeds: Vec<JsonConsoleTestbed>,
    pub benchmarks: Vec<JsonConsoleBenchmark>,
    pub variants: Vec<JsonConsoleVariant>,
    pub measures: Vec<JsonConsoleMeasure>,
    pub models: Vec<JsonConsoleModel>,
}

/// One line of a plot: one metric name of one measure of one variant, on one
/// branch and one testbed.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsolePerfLine {
    /// An index into `branches`.
    pub branch: u32,
    /// An index into `testbeds`.
    pub testbed: u32,
    /// An index into `benchmarks`.
    pub benchmark: u32,
    /// An index into `variants`.
    pub variant: u32,
    /// An index into `measures`.
    pub measure: u32,
    pub metric: MetricName,
    /// The model of the threshold that checked the line's latest checked point,
    /// an index into `models`. Absent when no threshold checked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<u32>,
    pub series: JsonConsoleSeries,
}

#[cfg(test)]
mod tests {
    use super::{JsonConsolePerfQuery, JsonConsolePerfQueryParams, MAX_DIMENSION_ENTRIES};
    use crate::{BenchmarkUuid, BranchUuid, MeasureUuid, TestbedUuid};

    fn query_params(metrics: Option<String>) -> JsonConsolePerfQueryParams {
        JsonConsolePerfQueryParams {
            branches: BranchUuid::new().to_string(),
            heads: None,
            testbeds: TestbedUuid::new().to_string(),
            specs: None,
            benchmarks: BenchmarkUuid::new().to_string(),
            parameters: None,
            measures: MeasureUuid::new().to_string(),
            metrics,
            start_time: None,
            end_time: None,
        }
    }

    #[test]
    fn truncates_the_metrics_list() {
        let metrics = (0..MAX_DIMENSION_ENTRIES + 2)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join(",");

        let query = JsonConsolePerfQuery::try_from(query_params(Some(metrics)))
            .expect("Failed to read the params");

        assert_eq!(
            query.metrics.map(|metrics| metrics.len()),
            Some(MAX_DIMENSION_ENTRIES)
        );
    }

    // An empty value has no list form, so it means every metric name rather than none.
    #[test]
    fn empty_metrics_draws_every_name() {
        let query = JsonConsolePerfQuery::try_from(query_params(Some(String::new())))
            .expect("Failed to read the params");

        assert!(query.metrics.is_none());
    }
}
