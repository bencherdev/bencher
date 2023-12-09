use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateBranch;
use bencher_json::{BranchName, JsonBranch, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::branch::CliBranchUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
    pub backend: Backend,
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

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonBranch = self
            .backend
            .send_with(|client| async move {
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
