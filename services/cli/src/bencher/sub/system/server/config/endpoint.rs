use std::convert::TryFrom;

use async_trait::async_trait;
use url::Url;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliConfigEndpoint,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub backend: Backend,
}

impl TryFrom<CliConfigEndpoint> for Endpoint {
    type Error = CliError;

    fn try_from(endpoint: CliConfigEndpoint) -> Result<Self, Self::Error> {
        let CliConfigEndpoint { backend } = endpoint;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Endpoint {
    async fn exec(&self) -> Result<(), CliError> {
        let _: Url = self
            .backend
            .send_with(
                |client| async move { client.server_config_endpoint_get().send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
