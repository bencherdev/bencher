#![allow(clippy::expect_used)]

use std::fmt;

use bencher_valid::{DateTime, NonEmpty, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

const MEASURE_NAME_ERROR: &str = "Failed to parse measure name.";
const MEASURE_SLUG_ERROR: &str = "Failed to parse measure slug.";
const MEASURE_UNITS_ERROR: &str = "Failed to parse measure units.";

pub const LATENCY_NAME_STR: &str = "Latency";
pub const LATENCY_SLUG_STR: &str = "latency";
pub const LATENCY_UNITS_STR: &str = "nanoseconds (ns)";

static LATENCY_NAME: Lazy<NonEmpty> =
    Lazy::new(|| LATENCY_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static LATENCY_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(LATENCY_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static LATENCY_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| LATENCY_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const THROUGHPUT_NAME_STR: &str = "Throughput";
pub const THROUGHPUT_SLUG_STR: &str = "throughput";
pub const THROUGHPUT_UNITS_STR: &str = "operations / second (ops/s)";

static THROUGHPUT_NAME: Lazy<NonEmpty> =
    Lazy::new(|| THROUGHPUT_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static THROUGHPUT_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(THROUGHPUT_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static THROUGHPUT_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| THROUGHPUT_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

// Iai measures

pub const INSTRUCTIONS_NAME_STR: &str = "Instructions";
pub const INSTRUCTIONS_SLUG_STR: &str = "instructions";
pub const INSTRUCTIONS_UNITS_STR: &str = "instructions";

static INSTRUCTIONS_NAME: Lazy<NonEmpty> =
    Lazy::new(|| INSTRUCTIONS_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static INSTRUCTIONS_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(INSTRUCTIONS_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static INSTRUCTIONS_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| INSTRUCTIONS_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const L1_ACCESSES_NAME_STR: &str = "L1 Accesses";
pub const L1_ACCESSES_SLUG_STR: &str = "l1-accesses";
pub const L1_ACCESSES_UNITS_STR: &str = "accesses";

static L1_ACCESSES_NAME: Lazy<NonEmpty> =
    Lazy::new(|| L1_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static L1_ACCESSES_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(L1_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static L1_ACCESSES_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| L1_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const L2_ACCESSES_NAME_STR: &str = "L2 Accesses";
pub const L2_ACCESSES_SLUG_STR: &str = "l2-accesses";
pub const L2_ACCESSES_UNITS_STR: &str = "accesses";

static L2_ACCESSES_NAME: Lazy<NonEmpty> =
    Lazy::new(|| L2_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static L2_ACCESSES_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(L2_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static L2_ACCESSES_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| L2_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));
pub const RAM_ACCESSES_NAME_STR: &str = "RAM Accesses";
pub const RAM_ACCESSES_SLUG_STR: &str = "ram-accesses";
pub const RAM_ACCESSES_UNITS_STR: &str = "accesses";

static RAM_ACCESSES_NAME: Lazy<NonEmpty> =
    Lazy::new(|| RAM_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static RAM_ACCESSES_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(RAM_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static RAM_ACCESSES_UNITS: Lazy<NonEmpty> =
    Lazy::new(|| RAM_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const ESTIMATED_CYCLES_NAME_STR: &str = "Estimated Cycles";
pub const ESTIMATED_CYCLES_SLUG_STR: &str = "estimated-cycles";
pub const ESTIMATED_CYCLES_UNITS_STR: &str = "estimated cycles";

static ESTIMATED_CYCLES_NAME: Lazy<NonEmpty> =
    Lazy::new(|| ESTIMATED_CYCLES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static ESTIMATED_CYCLES_SLUG: Lazy<Option<Slug>> =
    Lazy::new(|| Some(ESTIMATED_CYCLES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR)));
static ESTIMATED_CYCLES_UNITS: Lazy<NonEmpty> = Lazy::new(|| {
    ESTIMATED_CYCLES_UNITS_STR
        .parse()
        .expect(MEASURE_UNITS_ERROR)
});

crate::typed_uuid::typed_uuid!(MeasureUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMeasure {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub units: NonEmpty,
}

impl JsonNewMeasure {
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

    pub fn estimated_cycles() -> Self {
        Self {
            name: ESTIMATED_CYCLES_NAME.clone(),
            slug: ESTIMATED_CYCLES_SLUG.clone(),
            units: ESTIMATED_CYCLES_UNITS.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMeasures(pub Vec<JsonMeasure>);

crate::from_vec!(JsonMeasures[JsonMeasure]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMeasure {
    pub uuid: MeasureUuid,
    pub project: ProjectUuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub units: NonEmpty,
    pub created: DateTime,
    pub modified: DateTime,
}

impl fmt::Display for JsonMeasure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.units)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateMeasure {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub units: Option<NonEmpty>,
}
