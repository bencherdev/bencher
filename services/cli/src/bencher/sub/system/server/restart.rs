use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonRestart;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::CliRestart,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Restart {
    pub delay: Option<u64>,
    pub backend: AuthBackend,
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
        let _json =
            self.backend
                .as_ref()
                .send(|client| async move {
                    client.server_restart_post().body(self.clone()).send().await
                })
                .await?;
        Ok(())
    }
}
