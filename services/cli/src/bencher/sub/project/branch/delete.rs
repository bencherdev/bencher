use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::branch::CliBranchDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub backend: AuthBackend,
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

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_branch_delete()
                    .project(self.project.clone())
                    .branch(self.branch.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
