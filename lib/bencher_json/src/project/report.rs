use bencher_valid::GitHash;
use chrono::{DateTime, Utc};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    urlencoded::{from_millis, from_urlencoded, to_urlencoded, UrlEncodedError},
    JsonAlert, JsonMetricKind, JsonProject, JsonTestbed, JsonUser, ResourceId,
};

use super::{
    benchmark::JsonBenchmarkMetric, branch::JsonBranchVersion, threshold::JsonThresholdStatistic,
};

crate::typed_uuid::typed_uuid!(ReportUuid);

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewReport {
    pub branch: ResourceId,
    pub hash: Option<GitHash>,
    pub testbed: ResourceId,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub results: Vec<String>,
    pub settings: Option<JsonReportSettings>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportSettings {
    pub adapter: Option<JsonAdapter>,
    pub average: Option<JsonAverage>,
    pub fold: Option<JsonFold>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAdapter {
    #[default]
    Magic,
    Json,
    CSharp,
    CSharpDotNet,
    Cpp,
    CppCatch2,
    CppGoogle,
    Go,
    GoBench,
    Java,
    JavaJmh,
    Js,
    JsBenchmark,
    JsTime,
    Python,
    PythonAsv,
    PythonPytest,
    Ruby,
    RubyBenchmark,
    Rust,
    RustBench,
    RustCriterion,
    RustIai,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAverage {
    #[default]
    Mean,
    Median,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonFold {
    Min,
    Max,
    Mean,
    Median,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReports(pub Vec<JsonReport>);

crate::from_vec!(JsonReports[JsonReport]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid: ReportUuid,
    pub user: JsonUser,
    pub project: JsonProject,
    pub branch: JsonBranchVersion,
    pub testbed: JsonTestbed,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub adapter: JsonAdapter,
    pub results: JsonReportResults,
    pub alerts: JsonReportAlerts,
    pub created: DateTime<Utc>,
}

#[typeshare::typeshare]
pub type JsonReportResults = Vec<JsonReportIteration>;

#[typeshare::typeshare]
pub type JsonReportIteration = Vec<JsonReportResult>;

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportResult {
    pub metric_kind: JsonMetricKind,
    // The threshold should be the same for all the benchmark results
    pub threshold: Option<JsonThresholdStatistic>,
    pub benchmarks: Vec<JsonBenchmarkMetric>,
}

#[typeshare::typeshare]
pub type JsonReportAlerts = Vec<JsonAlert>;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportQueryParams {
    pub branch: Option<String>,
    pub testbed: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct JsonReportQuery {
    pub branch: Option<ResourceId>,
    pub testbed: Option<ResourceId>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl TryFrom<JsonReportQueryParams> for JsonReportQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonReportQueryParams) -> Result<Self, Self::Error> {
        let JsonReportQueryParams {
            branch,
            testbed,
            start_time,
            end_time,
        } = query_params;

        let branch = if let Some(branch) = branch {
            Some(from_urlencoded(&branch)?)
        } else {
            None
        };
        let testbed = if let Some(testbed) = testbed {
            Some(from_urlencoded(&testbed)?)
        } else {
            None
        };

        let start_time = if let Some(start_time) = start_time {
            Some(from_millis(start_time)?)
        } else {
            None
        };
        let end_time = if let Some(end_time) = end_time {
            Some(from_millis(end_time)?)
        } else {
            None
        };

        Ok(Self {
            branch,
            testbed,
            start_time,
            end_time,
        })
    }
}

impl JsonReportQuery {
    pub fn branch(&self) -> Option<String> {
        self.branch.as_ref().map(to_urlencoded)
    }

    pub fn testbed(&self) -> Option<String> {
        self.testbed.as_ref().map(to_urlencoded)
    }

    pub fn start_time(&self) -> Option<i64> {
        self.start_time.as_ref().map(DateTime::timestamp_millis)
    }

    pub fn end_time(&self) -> Option<i64> {
        self.end_time.as_ref().map(DateTime::timestamp_millis)
    }
}
