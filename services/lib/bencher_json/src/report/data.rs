use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JsonAdapter;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid: Uuid,
    pub user: Uuid,
    pub version: Uuid,
    pub testbed: Uuid,
    pub adapter: JsonAdapter,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub benchmarks: JsonReportBenchmarks,
    pub alerts: JsonReportAlerts,
}

pub type JsonReportBenchmarks = Vec<JsonReportBenchmark>;
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportBenchmark(pub Uuid);

pub type JsonReportAlerts = Vec<JsonReportAlert>;
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportAlert(pub Uuid);

impl From<Uuid> for JsonReportAlert {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}
