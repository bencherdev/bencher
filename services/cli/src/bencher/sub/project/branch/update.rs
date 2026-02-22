use bencher_client::types::JsonUpdateBranch;
use bencher_json::{BranchName, BranchResourceId, BranchSlug, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::branch::CliBranchUpdate,
};

use super::start_point::StartPoint;

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ProjectResourceId,
    pub branch: BranchResourceId,
    pub name: Option<BranchName>,
    pub slug: Option<BranchSlug>,
    pub start_point: StartPoint,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliBranchUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliBranchUpdate) -> Result<Self, Self::Error> {
        let CliBranchUpdate {
            project,
            branch,
            name,
            slug,
            start_point,
            archived,
            backend,
        } = update;
        Ok(Self {
            project,
            branch,
            name,
            slug,
            start_point: start_point.into(),
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateBranch {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            start_point,
            archived,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            start_point: start_point.into(),
            archived,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_branch_patch()
                    .project(self.project.clone())
                    .branch(self.branch.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
