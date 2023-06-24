use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonThreshold;

use super::benchmark::JsonBenchmarkMetric;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBoundary {
    // pub report: Uuid,
    // pub iteration: u32,
    // pub threshold: JsonThreshold,
    // pub benchmark: JsonBenchmarkMetric,
    pub left_side: Option<OrderedFloat<f64>>,
    pub right_side: Option<OrderedFloat<f64>>,
}
