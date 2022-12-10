use bencher_valid::{NonEmpty, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LATENCY_NAME_STR: &str = "Latency";
static LATENCY_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    LATENCY_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const LATENCY_SLUG_STR: &str = "latency";
static LATENCY_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        LATENCY_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const LATENCY_UNITS_STR: &str = "nanoseconds (ns)";
static LATENCY_UNITS: Lazy<Option<NonEmpty>> = Lazy::new(|| {
    Some(
        LATENCY_UNITS_STR
            .parse()
            .expect("Failed to parse metric kind units."),
    )
});
pub const DEFAULT_UNITS_STR: &str = "units";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetricKind {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub units: Option<NonEmpty>,
}

impl JsonNewMetricKind {
    pub fn latency() -> Self {
        Self {
            name: LATENCY_NAME.clone(),
            slug: LATENCY_SLUG.clone(),
            units: LATENCY_UNITS.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetricKind {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub units: NonEmpty,
}
