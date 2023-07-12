use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliVersion,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Version {
    pub backend: Backend,
}

impl TryFrom<CliVersion> for Version {
    type Error = CliError;

    fn try_from(version: CliVersion) -> Result<Self, Self::Error> {
        let CliVersion { backend } = version;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Version {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .send_with(
                |client| async move { client.server_version_get().send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
