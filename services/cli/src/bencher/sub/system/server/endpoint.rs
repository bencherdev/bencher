use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliEndpoint,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub backend: PubBackend,
}

impl TryFrom<CliEndpoint> for Endpoint {
    type Error = CliError;

    fn try_from(endpoint: CliEndpoint) -> Result<Self, Self::Error> {
        let CliEndpoint { backend } = endpoint;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Endpoint {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_endpoint_get().send().await })
            .await?;
        Ok(())
    }
}
