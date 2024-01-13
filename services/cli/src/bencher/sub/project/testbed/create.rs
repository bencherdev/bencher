use std::convert::TryFrom;

use bencher_client::types::JsonNewTestbed;
use bencher_json::{ResourceId, ResourceName, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: ResourceName,
    pub slug: Option<Slug>,
    pub soft: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliTestbedCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliTestbedCreate) -> Result<Self, Self::Error> {
        let CliTestbedCreate {
            project,
            name,
            slug,
            soft,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            soft,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create {
            name, slug, soft, ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            soft: Some(soft),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_testbed_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
