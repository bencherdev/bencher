use bencher_valid::Slug;
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LATENCY_NAME: &str = "Latency";
pub const LATENCY_SLUG_STR: &str = "latency";
static LATENCY_SLUG: Lazy<Option<Slug>> = Lazy::new(|| LATENCY_SLUG_STR.parse().ok());
pub const LATENCY_UNITS: &str = "nanoseconds (ns)";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetricKind {
    pub name: String,
    pub slug: Option<Slug>,
    pub units: Option<String>,
}

impl JsonNewMetricKind {
    pub fn latency() -> Self {
        Self {
            name: LATENCY_NAME.into(),
            slug: LATENCY_SLUG.clone(),
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
    pub slug: Slug,
    pub units: Option<String>,
}
