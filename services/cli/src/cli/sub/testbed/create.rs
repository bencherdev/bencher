use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::NewTestbed;

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
    pub os:         Option<String>,
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
            os,
            os_version,
            cpu,
            ram,
            disk,
            backend,
        } = create;
        Ok(Self {
            name,
            os,
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
        let testbed = NewTestbed::new(
            self.name.clone(),
            self.os.clone(),
            self.os_version.clone(),
            self.cpu.clone(),
            self.ram.clone(),
            self.disk.clone(),
        );
        self.backend.post(TESTBEDS_PATH, &testbed).await
    }
}
