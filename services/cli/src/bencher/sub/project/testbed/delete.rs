use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub testbed: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliTestbedDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliTestbedDelete) -> Result<Self, Self::Error> {
        let CliTestbedDelete {
            project,
            testbed,
            backend,
        } = delete;
        Ok(Self {
            project,
            testbed,
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
                    .proj_testbed_delete()
                    .project(self.project.clone())
                    .testbed(self.testbed.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
