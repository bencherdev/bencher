use bencher_valid::{NonEmpty, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LATENCY_NAME_STR: &str = "Latency";
#[allow(clippy::expect_used)]
static LATENCY_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    LATENCY_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const LATENCY_SLUG_STR: &str = "latency";
#[allow(clippy::expect_used)]
static LATENCY_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        LATENCY_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const LATENCY_UNITS_STR: &str = "nanoseconds (ns)";
#[allow(clippy::expect_used)]
static LATENCY_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    LATENCY_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});

pub const THROUGHPUT_NAME_STR: &str = "Throughput";
#[allow(clippy::expect_used)]
static THROUGHPUT_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    THROUGHPUT_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const THROUGHPUT_SLUG_STR: &str = "throughput";
#[allow(clippy::expect_used)]
static THROUGHPUT_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        THROUGHPUT_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const THROUGHPUT_UNITS_STR: &str = "operations / second (ops/s)";
#[allow(clippy::expect_used)]
static THROUGHPUT_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    THROUGHPUT_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetricKind {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub units: NonEmpty,
}

impl JsonNewMetricKind {
    pub fn latency() -> Self {
        Self {
            name: LATENCY_NAME.clone(),
            slug: LATENCY_SLUG.clone(),
            units: LATENCY_UNITS.clone(),
        }
    }

    pub fn throughput() -> Self {
        Self {
            name: THROUGHPUT_NAME.clone(),
            slug: THROUGHPUT_SLUG.clone(),
            units: THROUGHPUT_UNITS.clone(),
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
