use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonSpec;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::server::CliSpec,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Spec {
    pub backend: Backend,
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
        let _json: JsonSpec = self
            .backend
            .send_with(
                |client| async move { client.server_spec_get().send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
