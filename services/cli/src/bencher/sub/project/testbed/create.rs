use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonNewTestbed, NonEmpty, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::testbed::CliTestbedCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub backend: Backend,
}

impl TryFrom<CliTestbedCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliTestbedCreate) -> Result<Self, Self::Error> {
        let CliTestbedCreate {
            project,
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            project,
            name: NonEmpty::from_str(&name)?,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self { name, slug }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let testbed: JsonNewTestbed = self.clone().into();
        self.backend
            .post(&format!("/v0/projects/{}/testbeds", self.project), &testbed)
            .await?;
        Ok(())
    }
}
