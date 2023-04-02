use std::fmt;

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

pub const INSTRUCTIONS_NAME_STR: &str = "Instructions";
#[allow(clippy::expect_used)]
static INSTRUCTIONS_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    INSTRUCTIONS_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const INSTRUCTIONS_SLUG_STR: &str = "instructions";
#[allow(clippy::expect_used)]
static INSTRUCTIONS_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        INSTRUCTIONS_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const INSTRUCTIONS_UNITS_STR: &str = "instructions";
#[allow(clippy::expect_used)]
static INSTRUCTIONS_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    INSTRUCTIONS_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});
pub const CYCLES_NAME_STR: &str = "Cycles";
#[allow(clippy::expect_used)]
static CYCLES_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    CYCLES_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const CYCLES_SLUG_STR: &str = "cycles";
#[allow(clippy::expect_used)]
static CYCLES_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        CYCLES_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const CYCLES_UNITS_STR: &str = "cycles";
#[allow(clippy::expect_used)]
static CYCLES_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    CYCLES_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});
pub const L1_ACCESSES_NAME_STR: &str = "L1 Accesses";
#[allow(clippy::expect_used)]
static L1_ACCESSES_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    L1_ACCESSES_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const L1_ACCESSES_SLUG_STR: &str = "l1_accesses";
#[allow(clippy::expect_used)]
static L1_ACCESSES_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        L1_ACCESSES_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const L1_ACCESSES_UNITS_STR: &str = "accesses";
#[allow(clippy::expect_used)]
static L1_ACCESSES_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    L1_ACCESSES_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});
pub const L2_ACCESSES_NAME_STR: &str = "L2 Accesses";
#[allow(clippy::expect_used)]
static L2_ACCESSES_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    L2_ACCESSES_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const L2_ACCESSES_SLUG_STR: &str = "l2_accesses";
#[allow(clippy::expect_used)]
static L2_ACCESSES_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        L2_ACCESSES_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const L2_ACCESSES_UNITS_STR: &str = "accesses";
#[allow(clippy::expect_used)]
static L2_ACCESSES_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    L2_ACCESSES_UNITS_STR
        .parse()
        .expect("Failed to parse metric kind units.")
});
pub const RAM_ACCESSES_NAME_STR: &str = "RAM Accesses";
#[allow(clippy::expect_used)]
static RAM_ACCESSES_NAME: Lazy<NonEmpty> = Lazy::new(|| {
    RAM_ACCESSES_NAME_STR
        .parse()
        .expect("Failed to parse metric kind name.")
});
pub const RAM_ACCESSES_SLUG_STR: &str = "ram_accesses";
#[allow(clippy::expect_used)]
static RAM_ACCESSES_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        RAM_ACCESSES_SLUG_STR
            .parse()
            .expect("Failed to parse metric kind slug."),
    )
});
pub const RAM_ACCESSES_UNITS_STR: &str = "accesses";
#[allow(clippy::expect_used)]
static RAM_ACCESSES_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    RAM_ACCESSES_UNITS_STR
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
    pub fn instructions() -> Self {
        Self {
            name: INSTRUCTIONS_NAME.clone(),
            slug: INSTRUCTIONS_SLUG.clone(),
            units: INSTRUCTIONS_UNITS.clone(),
        }
    }
    pub fn cycles() -> Self {
        Self {
            name: CYCLES_NAME.clone(),
            slug: CYCLES_SLUG.clone(),
            units: CYCLES_UNITS.clone(),
        }
    }
    pub fn l1_accesses() -> Self {
        Self {
            name: L1_ACCESSES_NAME.clone(),
            slug: L1_ACCESSES_SLUG.clone(),
            units: L1_ACCESSES_UNITS.clone(),
        }
    }
    pub fn l2_accesses() -> Self {
        Self {
            name: L2_ACCESSES_NAME.clone(),
            slug: L2_ACCESSES_SLUG.clone(),
            units: L2_ACCESSES_UNITS.clone(),
        }
    }
    pub fn ram_accesses() -> Self {
        Self {
            name: RAM_ACCESSES_NAME.clone(),
            slug: RAM_ACCESSES_SLUG.clone(),
            units: RAM_ACCESSES_UNITS.clone(),
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

impl fmt::Display for JsonMetricKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.units)
    }
}
