use async_trait::async_trait;

use crate::{
    cli::{
        clap::CliTestbedCreate,
        sub::SubCmd,
        wide::Wide,
    },
    BencherError,
};

#[derive(Debug)]
pub struct Testbed {
    pub name:       String,
    pub os:         Option<String>,
    pub os_version: Option<String>,
    pub cpu:        Option<String>,
    pub ram:        Option<String>,
    pub disk:       Option<String>,
}

impl From<CliTestbedCreate> for Testbed {
    fn from(create: CliTestbedCreate) -> Self {
        let CliTestbedCreate {
            name,
            os,
            os_version,
            cpu,
            ram,
            disk,
        } = create;
        Self {
            name,
            os,
            os_version,
            cpu,
            ram,
            disk,
        }
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        todo!()
    }
}
