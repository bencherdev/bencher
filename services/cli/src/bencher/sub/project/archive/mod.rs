use bencher_json::ResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::archive::CliArchive,
};

mod action;
mod dimension;

pub use action::ArchiveAction;
use dimension::Dimension;

#[derive(Debug, Clone)]
pub struct Archive {
    pub project: ResourceId,
    pub dimension: Dimension,
    pub action: ArchiveAction,
    pub backend: AuthBackend,
}

#[derive(thiserror::Error, Debug)]
pub enum ArchiveError {
    #[error("Failed to parse UUID, slug, or name for the {dimension}: {err}")]
    ParseDimension {
        dimension: Dimension,
        err: bencher_json::ValidError,
    },
    #[error("Failed to query by dimension name ({name}): {err}")]
    GetDimension {
        name: String,
        err: crate::BackendError,
    },
    #[error(
        "Found {count} entries with name \"{name}\" in project \"{project}\"! Exactly one was expected.\nThis is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues"
    )]
    MultipleWithName {
        project: String,
        name: String,
        count: usize,
    },
    #[error("Could not find an entry with name \"{name}\" in project \"{project}\"")]
    NotFound { project: String, name: String },
    #[error("Failed to archive the {dimension} dimension: {err}")]
    ArchiveDimension {
        dimension: Dimension,
        err: crate::BackendError,
    },
}

impl TryFrom<(CliArchive, ArchiveAction)> for Archive {
    type Error = CliError;

    fn try_from((mock, action): (CliArchive, ArchiveAction)) -> Result<Self, Self::Error> {
        let CliArchive {
            project,
            dimension,
            backend,
        } = mock;
        Ok(Self {
            project,
            dimension: dimension.into(),
            action,
            backend: AuthBackend::try_from(backend)?.log(false),
        })
    }
}

impl SubCmd for Archive {
    async fn exec(&self) -> Result<(), CliError> {
        self.dimension
            .archive(&self.project, self.action, &self.backend)
            .await
            .map_err(Into::into)
    }
}
