#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::CliServerStats,
    CliError,
};

#[derive(Debug, Clone)]
pub struct ServerStats {
    pub backend: AuthBackend,
}

impl TryFrom<CliServerStats> for ServerStats {
    type Error = CliError;

    fn try_from(stats: CliServerStats) -> Result<Self, Self::Error> {
        let CliServerStats { backend } = stats;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for ServerStats {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_stats_get().send().await })
            .await?;
        Ok(())
    }
}
