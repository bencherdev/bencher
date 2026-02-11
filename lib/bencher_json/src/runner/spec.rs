use bencher_valid::{Cpu, DateTime, Disk, Memory};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(SpecUuid);

/// A hardware spec
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSpec {
    pub uuid: SpecUuid,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disk: Disk,
    pub network: bool,
    pub archived: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// List of specs
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSpecs(pub Vec<JsonSpec>);

crate::from_vec!(JsonSpecs[JsonSpec]);

/// Create a new spec
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewSpec {
    /// Number of CPUs
    pub cpu: Cpu,
    /// Memory size in bytes
    pub memory: Memory,
    /// Disk size in bytes
    pub disk: Disk,
    /// Whether the VM has network access
    #[serde(default)]
    pub network: bool,
}

/// Update a spec (archive/unarchive only)
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateSpec {
    /// Set whether the spec is archived.
    pub archived: Option<bool>,
}

/// Add a spec to a runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewRunnerSpec {
    /// The UUID of the spec to associate with the runner.
    pub spec: SpecUuid,
}
