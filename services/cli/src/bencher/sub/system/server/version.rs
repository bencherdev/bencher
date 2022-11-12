use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliVersion,
    CliError,
};

const VERSION_PATH: &str = "/v0/server/version";

#[derive(Debug, Clone)]
pub struct Version {
    pub backend: Backend,
}

impl TryFrom<CliVersion> for Version {
    type Error = CliError;

    fn try_from(version: CliVersion) -> Result<Self, Self::Error> {
        let CliVersion { host } = version;
        let backend = Backend::new(None, host)?;
        Ok(Self { backend })
    }
}

#[async_trait]
impl SubCmd for Version {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend.get(VERSION_PATH).await?;
        Ok(())
    }
}
