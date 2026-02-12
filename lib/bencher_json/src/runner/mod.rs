use bencher_valid::{DateTime, ResourceId, ResourceName, Secret, Slug};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::spec::SpecUuid;

pub mod job;
pub mod job_priority;
pub mod job_status;

pub use job::{
    JobUuid, JsonClaimJob, JsonJob, JsonJobConfig, JsonJobs, JsonUpdateJob, JsonUpdateJobResponse,
};
pub use job_priority::JobPriority;
pub use job_status::{JobStatus, JobUpdateStatus};

crate::typed_uuid::typed_uuid!(RunnerUuid);
crate::typed_slug::typed_slug!(RunnerSlug, ResourceName);

/// A runner UUID or slug.
#[typeshare::typeshare]
pub type RunnerResourceId = ResourceId<RunnerUuid, RunnerSlug>;

/// A benchmark runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunner {
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub specs: Vec<SpecUuid>,
    pub archived: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// List of runners
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunners(pub Vec<JsonRunner>);

crate::from_vec!(JsonRunners[JsonRunner]);

/// Create a new runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewRunner {
    /// The name of the runner.
    pub name: ResourceName,
    /// The preferred slug for the runner.
    /// If not provided, the slug will be generated from the name.
    pub slug: Option<Slug>,
}

/// Runner token response (returned on create or rotate)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunnerToken {
    pub uuid: RunnerUuid,
    /// The runner token. Only shown once - store it securely!
    pub token: Secret,
}

/// Update a runner
#[typeshare::typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateRunner {
    /// The new name for the runner.
    pub name: Option<ResourceName>,
    /// The new slug for the runner.
    pub slug: Option<Slug>,
    /// Set whether the runner is archived.
    pub archived: Option<bool>,
}
