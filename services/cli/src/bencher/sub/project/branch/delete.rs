use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::branch::CliBranchDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliBranchDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliBranchDelete) -> Result<Self, Self::Error> {
        let CliBranchDelete {
            project,
            branch,
            backend,
        } = delete;
        Ok(Self {
            project,
            branch,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonEmpty = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_branch_delete()
                        .project(self.project.clone())
                        .branch(self.branch.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
