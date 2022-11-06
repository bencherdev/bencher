#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod data;
pub mod new;

pub use data::JsonReport;
pub use new::{metric::JsonMetric, metrics_map::JsonMetricsMap, JsonNewReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonAdapter {
    Json,
    RustTest,
    RustBench,
}
