use bencher_client::{ClientError, types::Adapter};
use bencher_json::{BmfVersion, ProjectResourceId};
use serde::Deserialize;

use crate::bencher::{BackendError, backend::Backend};

const NOT_FOUND: u16 = 404;

/// The BMF version a report declares, and what set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredBmfVersion {
    /// An explicit `json_v0` or `json_v1` adapter, on any project.
    Adapter(BmfVersion),
    /// The project's version, or version 1 for a project the run creates.
    Project(BmfVersion),
}

#[derive(thiserror::Error, Debug)]
pub enum BmfVersionError {
    #[error("Failed to read the BMF version of the project: {0}")]
    Read(BackendError),
    #[error("Failed to read the BMF version of the project: {0}")]
    Parse(serde_json::Error),
}

impl DeclaredBmfVersion {
    /// The version an explicit `json_v0` or `json_v1` adapter declares, on any project.
    pub fn from_adapter(adapter: Option<Adapter>) -> Option<Self> {
        if matches!(adapter, Some(Adapter::JsonV0)) {
            Some(Self::Adapter(BmfVersion::V0))
        } else if matches!(adapter, Some(Adapter::JsonV1)) {
            Some(Self::Adapter(BmfVersion::V1))
        } else {
            None
        }
    }

    pub fn version(self) -> BmfVersion {
        match self {
            Self::Adapter(bmf_version) | Self::Project(bmf_version) => bmf_version,
        }
    }
}

/// The project's BMF version, read with the backend's retries.
/// A project that does not exist reads as version 1, the version a run creates it at.
pub async fn project_bmf_version(
    backend: &Backend,
    project: &ProjectResourceId,
) -> Result<BmfVersion, BmfVersionError> {
    let result = backend
        .send(|client| {
            let project = project.clone();
            async move { client.project_get().project(project).send().await }
        })
        .await;
    if let Err(err) = &result
        && is_not_found(err)
    {
        return Ok(BmfVersion::V1);
    }
    let ProjectBmfVersion { bmf_version } =
        serde_json::from_value(result.map_err(BmfVersionError::Read)?)
            .map_err(BmfVersionError::Parse)?;
    Ok(bmf_version)
}

fn is_not_found(err: &BackendError) -> bool {
    if let BackendError::Client(ClientError::ErrorResponse(response))
    | BackendError::ClientMismatch {
        err: ClientError::ErrorResponse(response),
        ..
    } = err
    {
        response.status.as_u16() == NOT_FOUND
    } else {
        false
    }
}

/// A server older than the field reads version 0 only.
#[derive(Deserialize)]
struct ProjectBmfVersion {
    #[serde(default)]
    bmf_version: BmfVersion,
}
