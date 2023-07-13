use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonThreshold;

use super::{benchmark::JsonBenchmarkMetric, boundary::JsonLimit};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlerts(pub Vec<JsonAlert>);

crate::from_vec!(JsonAlerts[JsonAlert]);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlert {
    pub uuid: Uuid,
    pub report: Uuid,
    pub iteration: u32,
    pub threshold: JsonThreshold,
    pub benchmark: JsonBenchmarkMetric,
    pub limit: JsonLimit,
    pub status: JsonAlertStatus,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAlertStatus {
    Unread,
    Read,
}
