use bencher_json::{ProjectResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::key::CliProjectKeyCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub name: ResourceName,
    pub ttl: Option<u32>,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectKeyCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliProjectKeyCreate) -> Result<Self, Self::Error> {
        let CliProjectKeyCreate {
            project,
            name,
            ttl,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            ttl,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_key_post()
                    .project(self.project.clone())
                    .body(bencher_client::types::JsonNewProjectKey {
                        name: self.name.clone().into(),
                        ttl: self.ttl,
                    })
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
