use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonEndpoint;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::server::CliEndpoint,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub backend: Backend,
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
        let _json: JsonEndpoint = self
            .backend
            .send_with(
                |client| async move { client.server_endpoint_get().send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
