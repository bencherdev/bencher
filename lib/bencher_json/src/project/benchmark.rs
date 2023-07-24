use std::fmt;

use bencher_valid::{BenchmarkName, Slug};
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

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
    pub slug: Slug,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
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
    pub slug: Slug,
    pub metric: JsonMetric,
    pub boundary: JsonBoundary,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBenchmark {
    pub name: BenchmarkName,
    pub slug: Option<Slug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateBenchmark {
    pub name: Option<BenchmarkName>,
    pub slug: Option<Slug>,
}
