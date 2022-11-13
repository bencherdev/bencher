use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonMetric;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonResult {
    pub uuid: Uuid,
    pub report: Uuid,
    pub iteration: u32,
    pub benchmark: Uuid,
    // pub branch: Uuid,
    // pub version_number: u32,
    // pub version_hash: Option<String>,
    // pub testbed: Uuid,
}
