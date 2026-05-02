use bencher_json::{ProjectKeyUuid, ProjectResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::key::CliProjectKeyUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ProjectResourceId,
    pub uuid: ProjectKeyUuid,
    pub name: Option<ResourceName>,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectKeyUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliProjectKeyUpdate) -> Result<Self, Self::Error> {
        let CliProjectKeyUpdate {
            project,
            uuid,
            name,
            backend,
        } = update;
        Ok(Self {
            project,
            uuid,
            name,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_key_patch()
                    .project(self.project.clone())
                    .key(self.uuid)
                    .body(bencher_client::types::JsonUpdateProjectKey {
                        name: self.name.clone().map(Into::into),
                    })
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
