use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ResourceId;

use super::metric::JsonMetric;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQuery {
    pub metric_kind: ResourceId,
    pub branches: Vec<Uuid>,
    pub testbeds: Vec<Uuid>,
    pub benchmarks: Vec<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub metric_kind: Uuid,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub results: Vec<JsonPerfMetrics>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetrics {
    pub branch: Uuid,
    pub testbed: Uuid,
    pub benchmark: Uuid,
    pub metrics: Vec<JsonPerfMetric>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfMetric {
    pub uuid: Uuid,
    pub iteration: u32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub version_number: u32,
    pub version_hash: Option<String>,
    pub metric: JsonMetric,
}
