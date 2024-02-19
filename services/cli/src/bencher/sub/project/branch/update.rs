use bencher_client::types::JsonUpdateBranch;
use bencher_json::{BranchName, ResourceId, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::branch::CliBranchUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
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
            backend,
        } = create;
        Ok(Self {
            project,
            branch,
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateBranch {
    fn from(update: Update) -> Self {
        let Update { name, slug, .. } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
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
