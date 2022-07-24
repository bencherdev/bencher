use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonTestbed;

use crate::{
    cli::{
        backend::Backend,
        clap::CliTestbedCreate,
        sub::SubCmd,
        wide::Wide,
    },
    BencherError,
};

const TESTBEDS_PATH: &str = "/v0/testbeds";

#[derive(Debug)]
pub struct Testbed {
    pub name:       String,
    pub os_name:    Option<String>,
    pub os_version: Option<String>,
    pub cpu:        Option<String>,
    pub ram:        Option<String>,
    pub disk:       Option<String>,
    pub backend:    Backend,
}

impl TryFrom<CliTestbedCreate> for Testbed {
    type Error = BencherError;

    fn try_from(create: CliTestbedCreate) -> Result<Self, Self::Error> {
        let CliTestbedCreate {
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
            backend,
        } = create;
        Ok(Self {
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let testbed = JsonTestbed {
            name:       self.name.clone(),
            os_name:    self.os_name.clone(),
            os_version: self.os_version.clone(),
            cpu:        self.cpu.clone(),
            ram:        self.ram.clone(),
            disk:       self.disk.clone(),
        };
        self.backend.post(TESTBEDS_PATH, &testbed).await
    }
}
