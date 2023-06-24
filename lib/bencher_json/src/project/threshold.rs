use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{JsonBranch, JsonMetricKind, JsonTestbed, ResourceId};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewThreshold {
    pub metric_kind: ResourceId,
    pub branch: ResourceId,
    pub testbed: ResourceId,
    #[serde(flatten)]
    pub statistic: JsonNewStatistic,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewStatistic {
    pub test: JsonStatisticKind,
    pub min_sample_size: Option<u32>,
    pub max_sample_size: Option<u32>,
    pub window: Option<u32>,
    pub left_side: Option<OrderedFloat<f64>>,
    pub right_side: Option<OrderedFloat<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThreshold {
    pub uuid: Uuid,
    pub metric_kind: JsonMetricKind,
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    pub statistic: JsonStatistic,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStatistic {
    pub uuid: Uuid,
    pub test: JsonStatisticKind,
    pub min_sample_size: Option<u32>,
    pub max_sample_size: Option<u32>,
    pub window: Option<u32>,
    pub left_side: Option<OrderedFloat<f64>>,
    pub right_side: Option<OrderedFloat<f64>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonStatisticKind {
    Z,
    T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholdStatistic {
    pub uuid: Uuid,
    pub statistic: JsonStatistic,
}
