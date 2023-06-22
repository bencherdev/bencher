use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonThreshold;

use super::benchmark::JsonBenchmarkMetric;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlert {
    pub uuid: Uuid,
    pub report: Uuid,
    pub iteration: u32,
    pub benchmark: JsonBenchmarkMetric,
    pub threshold: JsonThreshold,
    pub side: JsonSide,
    pub boundary: OrderedFloat<f32>,
    pub outlier: OrderedFloat<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonSide {
    Left,
    Right,
}
