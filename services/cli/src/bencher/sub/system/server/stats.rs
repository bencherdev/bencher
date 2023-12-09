#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonServerStats;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::server::CliServerStats,
    CliError,
};

#[derive(Debug, Clone)]
pub struct ServerStats {
    pub backend: Backend,
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
        let _json: JsonServerStats = self
            .backend
            .send_with(|client| async move { client.server_stats_get().send().await })
            .await?;
        Ok(())
    }
}
