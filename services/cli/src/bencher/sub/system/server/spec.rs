use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliSpec,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Spec {
    pub backend: PubBackend,
}

impl TryFrom<CliSpec> for Spec {
    type Error = CliError;

    fn try_from(spec: CliSpec) -> Result<Self, Self::Error> {
        let CliSpec { backend } = spec;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Spec {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_spec_get().send().await })
            .await?;
        Ok(())
    }
}
