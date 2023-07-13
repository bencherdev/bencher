use std::fmt;

use bencher_valid::BenchmarkName;
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonMetric;

use super::boundary::JsonBoundary;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarks(pub Vec<JsonBenchmark>);

crate::from_vec!(JsonBenchmarks[JsonBenchmark]);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
    pub created: DateTime<Utc>,
}

impl fmt::Display for JsonBenchmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarkMetric {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
    pub metric: JsonMetric,
    pub boundary: JsonBoundary,
    pub created: DateTime<Utc>,
}
