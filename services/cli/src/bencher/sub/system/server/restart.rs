use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonRestart;
use bencher_json::JsonEmpty;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliRestart,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Restart {
    pub delay: Option<u64>,
    pub backend: Backend,
}

impl TryFrom<CliRestart> for Restart {
    type Error = CliError;

    fn try_from(restart: CliRestart) -> Result<Self, Self::Error> {
        let CliRestart { delay, backend } = restart;
        Ok(Self {
            delay,
            backend: backend.try_into()?,
        })
    }
}

impl From<Restart> for JsonRestart {
    fn from(restart: Restart) -> Self {
        let Restart { delay, .. } = restart;
        Self { delay }
    }
}

#[async_trait]
impl SubCmd for Restart {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty=  self.backend
            .send_with(
                |client| async move { client.server_restart_post().body(self.clone()).send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
