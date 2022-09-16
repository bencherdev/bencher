#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod data;
pub mod new;

pub use data::JsonReport;
pub use new::{
    latency::JsonLatency, metrics_map::JsonMetricsMap, resource::JsonResource,
    throughput::JsonThroughput, JsonNewReport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAdapter {
    Json,
    RustTest,
    RustBench,
}
