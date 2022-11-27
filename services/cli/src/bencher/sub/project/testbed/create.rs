use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonNewTestbed, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::testbed::CliTestbedCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: String,
    pub slug: Option<Slug>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliTestbedCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliTestbedCreate) -> Result<Self, Self::Error> {
        let CliTestbedCreate {
            project,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            gpu,
            ram,
            disk,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            gpu,
            ram,
            disk,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create {
            project: _,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            gpu,
            ram,
            disk,
            backend: _,
        } = create;
        Self {
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            gpu,
            cpu,
            ram,
            disk,
        }
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
