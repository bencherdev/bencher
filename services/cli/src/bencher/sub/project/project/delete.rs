use bencher_json::ProjectResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::CliProjectDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ProjectResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectDelete> for Delete {
    type Error = CliError;

    fn try_from(view: CliProjectDelete) -> Result<Self, Self::Error> {
        let CliProjectDelete { project, backend } = view;
        Ok(Self {
            project,
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
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
