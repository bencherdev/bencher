use std::collections::HashMap;

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
    pub metrics: JsonMetrics,
}

pub type JsonMetrics = HashMap<Uuid, JsonMetric>;
