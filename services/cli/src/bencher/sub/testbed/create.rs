use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewTestbed, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::testbed::CliTestbedCreate,
    BencherError,
};

const TESTBEDS_PATH: &str = "/v0/testbeds";

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: String,
    pub slug: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliTestbedCreate> for Create {
    type Error = BencherError;

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
            ram,
            disk,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewTestbed {
    fn from(create: Create) -> Self {
        let Create {
            project,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
            backend: _,
        } = create;
        Self {
            project,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let testbed: JsonNewTestbed = self.clone().into();
        self.backend.post(TESTBEDS_PATH, &testbed).await?;
        Ok(())
    }
}
