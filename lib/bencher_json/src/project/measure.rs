#![allow(clippy::expect_used)]

use std::fmt;

use bencher_valid::{DateTime, ResourceName, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

const MEASURE_NAME_ERROR: &str = "Failed to parse measure name.";
const MEASURE_SLUG_ERROR: &str = "Failed to parse measure slug.";
const MEASURE_UNITS_ERROR: &str = "Failed to parse measure units.";

pub const MEASURE_UNITS_STR: &str = "Measure (units)";
pub static MEASURE_UNITS: Lazy<ResourceName> =
    Lazy::new(|| MEASURE_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const LATENCY_NAME_STR: &str = "Latency";
pub const LATENCY_SLUG_STR: &str = "latency";
pub const LATENCY_UNITS_STR: &str = "nanoseconds (ns)";

static LATENCY_NAME: Lazy<ResourceName> =
    Lazy::new(|| LATENCY_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static LATENCY_SLUG: Lazy<Slug> = Lazy::new(|| LATENCY_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static LATENCY_UNITS: Lazy<ResourceName> =
    Lazy::new(|| LATENCY_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const THROUGHPUT_NAME_STR: &str = "Throughput";
pub const THROUGHPUT_SLUG_STR: &str = "throughput";
pub const THROUGHPUT_UNITS_STR: &str = "operations / second (ops/s)";

static THROUGHPUT_NAME: Lazy<ResourceName> =
    Lazy::new(|| THROUGHPUT_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static THROUGHPUT_SLUG: Lazy<Slug> =
    Lazy::new(|| THROUGHPUT_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static THROUGHPUT_UNITS: Lazy<ResourceName> =
    Lazy::new(|| THROUGHPUT_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

// Iai measures

pub const INSTRUCTIONS_NAME_STR: &str = "Instructions";
pub const INSTRUCTIONS_SLUG_STR: &str = "instructions";
pub const INSTRUCTIONS_UNITS_STR: &str = "instructions";

static INSTRUCTIONS_NAME: Lazy<ResourceName> =
    Lazy::new(|| INSTRUCTIONS_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static INSTRUCTIONS_SLUG: Lazy<Slug> =
    Lazy::new(|| INSTRUCTIONS_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static INSTRUCTIONS_UNITS: Lazy<ResourceName> =
    Lazy::new(|| INSTRUCTIONS_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const L1_ACCESSES_NAME_STR: &str = "L1 Accesses";
pub const L1_ACCESSES_SLUG_STR: &str = "l1-accesses";
pub const L1_ACCESSES_UNITS_STR: &str = "accesses";

static L1_ACCESSES_NAME: Lazy<ResourceName> =
    Lazy::new(|| L1_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static L1_ACCESSES_SLUG: Lazy<Slug> =
    Lazy::new(|| L1_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static L1_ACCESSES_UNITS: Lazy<ResourceName> =
    Lazy::new(|| L1_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const L2_ACCESSES_NAME_STR: &str = "L2 Accesses";
pub const L2_ACCESSES_SLUG_STR: &str = "l2-accesses";
pub const L2_ACCESSES_UNITS_STR: &str = "accesses";

static L2_ACCESSES_NAME: Lazy<ResourceName> =
    Lazy::new(|| L2_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static L2_ACCESSES_SLUG: Lazy<Slug> =
    Lazy::new(|| L2_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static L2_ACCESSES_UNITS: Lazy<ResourceName> =
    Lazy::new(|| L2_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const RAM_ACCESSES_NAME_STR: &str = "RAM Accesses";
pub const RAM_ACCESSES_SLUG_STR: &str = "ram-accesses";
pub const RAM_ACCESSES_UNITS_STR: &str = "accesses";

static RAM_ACCESSES_NAME: Lazy<ResourceName> =
    Lazy::new(|| RAM_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static RAM_ACCESSES_SLUG: Lazy<Slug> =
    Lazy::new(|| RAM_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static RAM_ACCESSES_UNITS: Lazy<ResourceName> =
    Lazy::new(|| RAM_ACCESSES_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const TOTAL_ACCESSES_NAME_STR: &str = "Total Accesses";
pub const TOTAL_ACCESSES_SLUG_STR: &str = "total-accesses";
pub const TOTAL_ACCESSES_UNITS_STR: &str = "accesses";

static TOTAL_ACCESSES_NAME: Lazy<ResourceName> =
    Lazy::new(|| TOTAL_ACCESSES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static TOTAL_ACCESSES_SLUG: Lazy<Slug> =
    Lazy::new(|| TOTAL_ACCESSES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static TOTAL_ACCESSES_UNITS: Lazy<ResourceName> =
    Lazy::new(|| TOTAL_ACCESSES_SLUG_STR.parse().expect(MEASURE_UNITS_ERROR));

pub const ESTIMATED_CYCLES_NAME_STR: &str = "Estimated Cycles";
pub const ESTIMATED_CYCLES_SLUG_STR: &str = "estimated-cycles";
pub const ESTIMATED_CYCLES_UNITS_STR: &str = "estimated cycles";

static ESTIMATED_CYCLES_NAME: Lazy<ResourceName> =
    Lazy::new(|| ESTIMATED_CYCLES_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
static ESTIMATED_CYCLES_SLUG: Lazy<Slug> =
    Lazy::new(|| ESTIMATED_CYCLES_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static ESTIMATED_CYCLES_UNITS: Lazy<ResourceName> = Lazy::new(|| {
    ESTIMATED_CYCLES_UNITS_STR
        .parse()
        .expect(MEASURE_UNITS_ERROR)
});

// File size measures

pub const FILE_SIZE_NAME_STR: &str = "File Size";
pub const FILE_SIZE_SLUG_STR: &str = "file-size";
pub const FILE_SIZE_UNITS_STR: &str = "bytes (B)";

static FILE_SIZE_NAME: Lazy<ResourceName> =
    Lazy::new(|| FILE_SIZE_NAME_STR.parse().expect(MEASURE_NAME_ERROR));
pub static FILE_SIZE_SLUG: Lazy<Slug> =
    Lazy::new(|| FILE_SIZE_SLUG_STR.parse().expect(MEASURE_SLUG_ERROR));
static FILE_SIZE_UNITS: Lazy<ResourceName> =
    Lazy::new(|| FILE_SIZE_UNITS_STR.parse().expect(MEASURE_UNITS_ERROR));

crate::typed_uuid::typed_uuid!(MeasureUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMeasure {
    /// The name of the measure.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The preferred slug for the measure.
    /// If not provided, the slug will be generated from the name.
    /// If the provided or generated slug is already in use, a unique slug will be generated.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// The units of measure.
    /// Maximum length is 64 characters.
    pub units: ResourceName,
}

impl JsonNewMeasure {
    pub fn latency() -> Self {
        Self {
            name: LATENCY_NAME.clone(),
            slug: Some(LATENCY_SLUG.clone()),
            units: LATENCY_UNITS.clone(),
        }
    }

    pub fn throughput() -> Self {
        Self {
            name: THROUGHPUT_NAME.clone(),
            slug: Some(THROUGHPUT_SLUG.clone()),
            units: THROUGHPUT_UNITS.clone(),
        }
    }

    pub fn instructions() -> Self {
        Self {
            name: INSTRUCTIONS_NAME.clone(),
            slug: Some(INSTRUCTIONS_SLUG.clone()),
            units: INSTRUCTIONS_UNITS.clone(),
        }
    }

    pub fn l1_accesses() -> Self {
        Self {
            name: L1_ACCESSES_NAME.clone(),
            slug: Some(L1_ACCESSES_SLUG.clone()),
            units: L1_ACCESSES_UNITS.clone(),
        }
    }

    pub fn l2_accesses() -> Self {
        Self {
            name: L2_ACCESSES_NAME.clone(),
            slug: Some(L2_ACCESSES_SLUG.clone()),
            units: L2_ACCESSES_UNITS.clone(),
        }
    }

    pub fn ram_accesses() -> Self {
        Self {
            name: RAM_ACCESSES_NAME.clone(),
            slug: Some(RAM_ACCESSES_SLUG.clone()),
            units: RAM_ACCESSES_UNITS.clone(),
        }
    }

    pub fn total_accesses() -> Self {
        Self {
            name: TOTAL_ACCESSES_NAME.clone(),
            slug: Some(TOTAL_ACCESSES_SLUG.clone()),
            units: TOTAL_ACCESSES_UNITS.clone(),
        }
    }

    pub fn estimated_cycles() -> Self {
        Self {
            name: ESTIMATED_CYCLES_NAME.clone(),
            slug: Some(ESTIMATED_CYCLES_SLUG.clone()),
            units: ESTIMATED_CYCLES_UNITS.clone(),
        }
    }

    pub fn file_size() -> Self {
        Self {
            name: FILE_SIZE_NAME.clone(),
            slug: Some(FILE_SIZE_SLUG.clone()),
            units: FILE_SIZE_UNITS.clone(),
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
    pub name: ResourceName,
    pub slug: Slug,
    pub units: ResourceName,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonMeasure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.units)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateMeasure {
    /// The new name of the measure.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
    /// The preferred new slug for the measure.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// The new units of measure.
    /// Maximum length is 64 characters.
    pub units: Option<ResourceName>,
    /// Set whether the measure is archived.
    pub archived: Option<bool>,
}
