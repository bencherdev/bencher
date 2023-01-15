use bencher_valid::NonEmpty;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type BenchmarkName = NonEmpty;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BenchmarkName,
}
