use bencher_valid::{BenchmarkName, BranchName, DateTimeMillis, GitHash, ResourceName, Search};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BenchmarkSlug, BenchmarkUuid, BranchSlug, BranchUuid, JsonDirection, MeasureSlug, MeasureUuid,
    TestbedSlug, TestbedUuid,
};

/// How many rows a page of a console dimension list holds when the request does not say.
pub const DEFAULT_CONSOLE_DIMENSIONS_PER_PAGE: u8 = 64;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleDimensionsQueryParams {
    /// Search by name, slug, or UUID.
    pub search: Option<Search>,
    /// If set to `true`, only the archived ones. Otherwise, only the active ones.
    pub archived: Option<bool>,
    /// The order of the rows, by name when not given.
    pub sort: Option<ConsoleDimensionsSort>,
    /// Ascending by name and descending by time when not given.
    pub direction: Option<JsonDirection>,
    /// The page to return, starting at 1.
    pub page: Option<u32>,
    /// The number of rows to skip, instead of a page.
    pub offset: Option<u32>,
    /// The number of rows per page, 64 when not given.
    pub per_page: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConsoleDimensionsSort {
    #[default]
    Name,
    Created,
    /// By the newest report, with the never reported last either way.
    LastUsed,
}

/// A page of a project's branches as the console lists them.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBranches {
    /// The branches that match the request, over every page.
    pub total: u32,
    /// Every active branch, whatever the search.
    pub active: u32,
    /// Every archived branch, whatever the search.
    pub archived: u32,
    pub branches: Vec<JsonConsoleBranchRow>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBranchRow {
    pub uuid: BranchUuid,
    pub name: BranchName,
    pub slug: BranchSlug,
    /// The branch its current head started from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_point: Option<BranchName>,
    /// The hash of the version its newest report ran on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<GitHash>,
    pub created: DateTimeMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
    /// When the API took its newest report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_report: Option<DateTimeMillis>,
    /// The thresholds on it whose testbed and measure are active:
    /// active while it is, archived with it, and back when it is unarchived.
    pub thresholds: u32,
    /// The thresholds on it that an archived testbed or measure holds archived either way.
    pub held_thresholds: u32,
}

/// A page of a project's testbeds as the console lists them.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleTestbeds {
    /// The testbeds that match the request, over every page.
    pub total: u32,
    /// Every active testbed, whatever the search.
    pub active: u32,
    /// Every archived testbed, whatever the search.
    pub archived: u32,
    pub testbeds: Vec<JsonConsoleTestbedRow>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleTestbedRow {
    pub uuid: TestbedUuid,
    pub name: ResourceName,
    pub slug: TestbedSlug,
    /// The name of the spec it runs on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<ResourceName>,
    pub created: DateTimeMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
    /// When the API took the report that ended last on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_report: Option<DateTimeMillis>,
    /// The thresholds on it whose branch and measure are active:
    /// active while it is, archived with it, and back when it is unarchived.
    pub thresholds: u32,
    /// The thresholds on it that an archived branch or measure holds archived either way.
    pub held_thresholds: u32,
}

/// A page of a project's benchmarks as the console lists them.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBenchmarks {
    /// The benchmarks that match the request, over every page.
    pub total: u32,
    /// Every active benchmark, whatever the search.
    pub active: u32,
    /// Every archived benchmark, whatever the search.
    pub archived: u32,
    pub benchmarks: Vec<JsonConsoleBenchmarkRow>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleBenchmarkRow {
    pub uuid: BenchmarkUuid,
    pub name: BenchmarkName,
    pub slug: BenchmarkSlug,
    /// Its active variants that have a report.
    pub variants: u32,
    pub created: DateTimeMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
    /// When the API took its newest report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_report: Option<DateTimeMillis>,
}

/// A page of a project's measures as the console lists them.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleMeasures {
    /// The measures that match the request, over every page.
    pub total: u32,
    /// Every active measure, whatever the search.
    pub active: u32,
    /// Every archived measure, whatever the search.
    pub archived: u32,
    pub measures: Vec<JsonConsoleMeasureRow>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleMeasureRow {
    pub uuid: MeasureUuid,
    pub name: ResourceName,
    pub slug: MeasureSlug,
    pub units: ResourceName,
    pub created: DateTimeMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
    /// When the API took the report of the newest value stored for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_report: Option<DateTimeMillis>,
    /// The thresholds on it whose branch and testbed are active:
    /// active while it is, archived with it, and back when it is unarchived.
    pub thresholds: u32,
    /// The thresholds on it that an archived branch or testbed holds archived either way.
    pub held_thresholds: u32,
}
