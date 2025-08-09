use std::{fmt, sync::LazyLock};

use bencher_valid::{DateTime, NameId, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

pub mod built_in;

#[expect(clippy::expect_used)]
pub static DEFAULT_UNIT: LazyLock<ResourceName> = LazyLock::new(|| {
    "Measure (units)"
        .parse()
        .expect("Failed to parse measure units.")
});

crate::typed_uuid::typed_uuid!(MeasureUuid);
crate::typed_slug::typed_slug!(MeasureSlug, ResourceName);

/// A measure UUID, slug, or name.
#[typeshare::typeshare]
pub type MeasureNameId = NameId<MeasureUuid, MeasureSlug, ResourceName>;

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
    pub slug: Option<MeasureSlug>,
    /// The units of measure.
    /// Maximum length is 64 characters.
    pub units: ResourceName,
}

impl JsonNewMeasure {
    pub fn generic_unit() -> ResourceName {
        DEFAULT_UNIT.clone()
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
    pub slug: MeasureSlug,
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
    pub slug: Option<MeasureSlug>,
    /// The new units of measure.
    /// Maximum length is 64 characters.
    pub units: Option<ResourceName>,
    /// Set whether the measure is archived.
    pub archived: Option<bool>,
}
