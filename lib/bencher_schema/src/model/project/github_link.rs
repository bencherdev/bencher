#[cfg(feature = "plus")]
use bencher_github_client::GitHubApp;
#[cfg(feature = "plus")]
use bencher_json::GitHubRepositoryName;
use bencher_json::project::GitHubLinkStatus;
use dropshot::HttpError;

use super::ProjectId;
use crate::context::DbConnection;
#[cfg(feature = "plus")]
use crate::{error::issue_error, model::project::linked_repository::QueryLinkedRepository};

/// Whether the server can link a project to a GitHub repository, which takes the GitHub App.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubLinking {
    #[cfg(feature = "plus")]
    Available,
    Unavailable,
}

#[cfg(feature = "plus")]
impl GitHubLinking {
    pub fn new(github_app: Option<&GitHubApp>) -> Self {
        if github_app.is_some() {
            Self::Available
        } else {
            Self::Unavailable
        }
    }
}

/// A project's link to GitHub, as far as the server can make one.
#[derive(Debug, Clone)]
pub enum GitHubLink {
    Unavailable,
    #[cfg(feature = "plus")]
    Unlinked,
    #[cfg(feature = "plus")]
    Linked(QueryLinkedRepository),
}

impl GitHubLink {
    /// The project's link, which costs one lookup by its primary key when the server can link.
    #[cfg(feature = "plus")]
    pub fn get(
        conn: &mut DbConnection,
        github: GitHubLinking,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Ok(match github {
            GitHubLinking::Available => QueryLinkedRepository::get(conn, project_id)
                .map_err(|e| {
                    let message =
                        format!("Failed to query linked_repository table with ID ({project_id})");
                    issue_error(&message, &message, e)
                })?
                .map_or(Self::Unlinked, Self::Linked),
            GitHubLinking::Unavailable => Self::Unavailable,
        })
    }

    #[cfg(not(feature = "plus"))]
    pub fn get(
        _conn: &mut DbConnection,
        _github: GitHubLinking,
        _project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Ok(Self::Unavailable)
    }

    pub fn status(&self) -> Option<GitHubLinkStatus> {
        match self {
            Self::Unavailable => None,
            #[cfg(feature = "plus")]
            Self::Unlinked => Some(GitHubLinkStatus::Unlinked),
            #[cfg(feature = "plus")]
            Self::Linked(link) => Some(link.status.into()),
        }
    }

    #[cfg(feature = "plus")]
    pub fn full_name(&self) -> Option<&GitHubRepositoryName> {
        if let Self::Linked(link) = self {
            Some(&link.full_name)
        } else {
            None
        }
    }
}
