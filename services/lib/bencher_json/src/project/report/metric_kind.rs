#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LATENCY_NAME: &str = "Latency";
pub const LATENCY_SLUG: &str = "latency";
pub const LATENCY_UNITS: &str = "nanoseconds (ns)";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetricKind {
    pub name: String,
    pub slug: Option<String>,
    pub units: Option<String>,
}

impl JsonNewMetricKind {
    pub fn latency() -> Self {
        Self {
            name: LATENCY_NAME.into(),
            slug: Some(LATENCY_SLUG.into()),
            units: Some(LATENCY_UNITS.into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetricKind {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: String,
    pub units: Option<String>,
}
