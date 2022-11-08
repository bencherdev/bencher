use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JsonAdapter;

pub mod benchmarks;
pub mod mean;
pub mod median;
pub mod metric;
pub mod metrics;
pub mod metrics_map;

pub use benchmarks::{JsonBenchmarks, JsonBenchmarksMap};
pub use metrics::JsonMetrics;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewReport {
    pub branch: Uuid,
    pub hash: Option<String>,
    pub testbed: Uuid,
    pub adapter: JsonAdapter,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub benchmarks: JsonBenchmarks,
}
