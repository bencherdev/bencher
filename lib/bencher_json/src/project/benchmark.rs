use bencher_valid::BenchmarkName;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(debug_assertions, derive(Default))]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
}
