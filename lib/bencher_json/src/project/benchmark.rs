use std::fmt;

use bencher_valid::{BenchmarkName, DateTime, Slug};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

crate::typed_uuid::typed_uuid!(BenchmarkUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBenchmark {
    /// The name of the benchmark.
    /// Maximum length is 1,024 characters.
    pub name: BenchmarkName,
    /// The preferred slug for the benchmark.
    /// If not provided, the slug will be generated from the name.
    /// If the provided or generated slug is already in use, a unique slug will be generated.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarks(pub Vec<JsonBenchmark>);

crate::from_vec!(JsonBenchmarks[JsonBenchmark]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: BenchmarkUuid,
    pub project: ProjectUuid,
    pub name: BenchmarkName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonBenchmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateBenchmark {
    /// The new name of the benchmark.
    /// Maximum length is 1,024 characters.
    pub name: Option<BenchmarkName>,
    /// The preferred new slug for the benchmark.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// Set whether the benchmark is archived.
    pub archived: Option<bool>,
}
