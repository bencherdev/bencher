use bencher_json::{ProjectKeyUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::key::CliProjectKeyRevoke,
};

#[derive(Debug, Clone)]
pub struct Revoke {
    pub project: ProjectResourceId,
    pub uuid: ProjectKeyUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectKeyRevoke> for Revoke {
    type Error = CliError;

    fn try_from(revoke: CliProjectKeyRevoke) -> Result<Self, Self::Error> {
        let CliProjectKeyRevoke {
            project,
            uuid,
            backend,
        } = revoke;
        Ok(Self {
            project,
            uuid,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Revoke {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_key_delete()
                    .project(self.project.clone())
                    .key(self.uuid)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
