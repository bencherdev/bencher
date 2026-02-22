use bencher_client::types::JsonNewTestbed;
#[cfg(feature = "plus")]
use bencher_json::SpecResourceId;
use bencher_json::{ProjectResourceId, ResourceName, TestbedSlug};

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
    #[cfg(feature = "plus")]
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
            #[cfg(feature = "plus")]
            spec,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create {
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
            ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            #[cfg(feature = "plus")]
            spec: spec.map(Into::into),
            #[cfg(not(feature = "plus"))]
            spec: None,
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
