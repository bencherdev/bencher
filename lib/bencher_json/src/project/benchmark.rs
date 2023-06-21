use std::fmt;

use bencher_valid::{BenchmarkName, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::JsonMetric;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
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
    pub url: Url,
}
