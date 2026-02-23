use bencher_json::ProjectResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::CliProjectDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ProjectResourceId,
    pub hard: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliProjectDelete) -> Result<Self, Self::Error> {
        let CliProjectDelete {
            project,
            hard,
            backend,
        } = delete;
        Ok(Self {
            project,
            hard,
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
                    .project_delete()
                    .project(self.project.clone())
                    .hard(self.hard)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
