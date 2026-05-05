use bencher_json::{ProjectKeyUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::key::CliProjectKeyView,
};

#[derive(Debug, Clone)]
pub struct View {
    pub project: ProjectResourceId,
    pub uuid: ProjectKeyUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectKeyView> for View {
    type Error = CliError;

    fn try_from(view: CliProjectKeyView) -> Result<Self, Self::Error> {
        let CliProjectKeyView {
            project,
            uuid,
            backend,
        } = view;
        Ok(Self {
            project,
            uuid,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_key_get()
                    .project(self.project.clone())
                    .key(self.uuid)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
