use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::branch::CliBranchView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliBranchView> for View {
    type Error = CliError;

    fn try_from(view: CliBranchView) -> Result<Self, Self::Error> {
        let CliBranchView {
            project,
            branch,
            backend,
        } = view;
        Ok(Self {
            project,
            branch,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_branch_get()
                    .project(self.project.clone())
                    .branch(self.branch.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
