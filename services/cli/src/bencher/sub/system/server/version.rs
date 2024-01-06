use std::convert::TryFrom;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliVersion,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Version {
    pub backend: PubBackend,
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

impl SubCmd for Version {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_version_get().send().await })
            .await?;
        Ok(())
    }
}
