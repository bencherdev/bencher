use bencher_valid::{Architecture, Cpu, DateTime, Disk, Memory, ResourceId, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(SpecUuid);
crate::typed_slug::typed_slug!(SpecSlug, ResourceName);

/// A spec UUID or slug.
#[typeshare::typeshare]
pub type SpecResourceId = ResourceId<SpecUuid, SpecSlug>;

/// Create a new spec
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewSpec {
    /// The name of the spec.
    pub name: ResourceName,
    /// The preferred slug for the spec.
    /// If not provided, the slug will be generated from the name.
    pub slug: Option<SpecSlug>,
    /// CPU architecture
    pub architecture: Architecture,
    /// Number of CPUs
    pub cpu: Cpu,
    /// Memory size in bytes
    pub memory: Memory,
    /// Disk size in bytes
    pub disk: Disk,
    /// Whether the VM has network access
    #[serde(default)]
    pub network: bool,
    /// Whether this spec is the fallback spec
    #[serde(default)]
    pub fallback: bool,
}

/// List of specs
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSpecs(pub Vec<JsonSpec>);

crate::from_vec!(JsonSpecs[JsonSpec]);

/// A hardware spec
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSpec {
    pub uuid: SpecUuid,
    pub name: ResourceName,
    pub slug: SpecSlug,
    /// CPU architecture
    pub architecture: Architecture,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disk: Disk,
    pub network: bool,
    pub fallback: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

/// Update a spec
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateSpec {
    /// The new name for the spec.
    pub name: Option<ResourceName>,
    /// The new slug for the spec.
    pub slug: Option<SpecSlug>,
    /// Set whether the spec is the fallback spec.
    pub fallback: Option<bool>,
    /// Set whether the spec is archived.
    pub archived: Option<bool>,
}

/// Add a spec to a runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewRunnerSpec {
    /// The UUID or slug of the spec to associate with the runner.
    pub spec: SpecResourceId,
}
