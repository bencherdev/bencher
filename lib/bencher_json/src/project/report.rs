use bencher_valid::GitHash;
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ResourceId;

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
    pub fold: Option<JsonFold>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAdapter {
    #[default]
    Magic,
    Json,
    Rust,
    RustBench,
    RustCriterion,
    Cpp,
    CppGoogle,
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
pub struct JsonReport {
    pub uuid: Uuid,
    pub user: Uuid,
    pub version: Uuid,
    pub testbed: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub adapter: JsonAdapter,
    pub results: JsonReportResults,
    pub alerts: JsonReportAlerts,
}

pub type JsonReportResults = Vec<JsonReportResult>;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportResult(pub Uuid);

impl From<Uuid> for JsonReportResult {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

pub type JsonReportAlerts = Vec<JsonReportAlert>;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportAlert(pub Uuid);

impl From<Uuid> for JsonReportAlert {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
