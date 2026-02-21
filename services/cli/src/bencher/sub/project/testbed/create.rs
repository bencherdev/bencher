use bencher_client::types::JsonNewTestbed;
use bencher_json::{ProjectResourceId, ResourceName, SpecResourceId, TestbedSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub name: ResourceName,
    pub slug: Option<TestbedSlug>,
    pub spec: Option<SpecResourceId>,
    pub backend: AuthBackend,
}

impl TryFrom<CliTestbedCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliTestbedCreate) -> Result<Self, Self::Error> {
        let CliTestbedCreate {
            project,
            name,
            slug,
            spec,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create {
            name, slug, spec, ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            spec: spec.map(Into::into),
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
