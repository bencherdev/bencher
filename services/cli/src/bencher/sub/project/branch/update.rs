use bencher_client::types::{JsonUpdateBranch, JsonUpdateStartPoint};
use bencher_json::{BranchName, GitHash, NameId, ResourceId, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::branch::{CliBranchUpdate, CliStartPointUpdate},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
    pub start_point_branch: Option<NameId>,
    pub start_point_hash: Option<GitHash>,
    pub start_point_max_versions: u32,
    pub start_point_clone_thresholds: bool,
    pub start_point_reset: bool,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliBranchUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliBranchUpdate) -> Result<Self, Self::Error> {
        let CliBranchUpdate {
            project,
            branch,
            name,
            slug,
            start_point,
            archived,
            backend,
        } = create;
        let CliStartPointUpdate {
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
        } = start_point;
        Ok(Self {
            project,
            branch,
            name,
            slug,
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
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
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
            archived,
            ..
        } = update;
        let start_point =
            (start_point_branch.is_some() || start_point_reset).then_some(JsonUpdateStartPoint {
                branch: start_point_branch.map(Into::into),
                hash: start_point_hash.map(Into::into),
                max_versions: Some(start_point_max_versions),
                clone_thresholds: Some(start_point_clone_thresholds),
                reset: Some(start_point_reset),
            });
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            start_point,
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
