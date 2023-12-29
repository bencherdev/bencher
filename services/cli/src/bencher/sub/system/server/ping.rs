use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliPing,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Ping {
    pub backend: PubBackend,
}

impl TryFrom<CliPing> for Ping {
    type Error = CliError;

    fn try_from(ping: CliPing) -> Result<Self, Self::Error> {
        let CliPing { backend } = ping;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Ping {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move { client.server_ping_get().send().await })
            .await?;
        Ok(())
    }
}
