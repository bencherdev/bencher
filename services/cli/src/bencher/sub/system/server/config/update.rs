use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonConfig, JsonUpdateConfig};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::CliConfigUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub config: Box<JsonConfig>,
    pub delay: Option<u64>,
    pub backend: AuthBackend,
}

impl TryFrom<CliConfigUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliConfigUpdate) -> Result<Self, Self::Error> {
        let CliConfigUpdate {
            config,
            delay,
            backend,
        } = update;
        Ok(Self {
            config: serde_json::from_str(&config).map_err(CliError::SerializeConfig)?,
            delay,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateConfig {
    fn from(update: Update) -> Self {
        let Update { config, delay, .. } = update;
        Self {
            config: *config,
            delay,
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(
                |client| async move { client.server_config_put().body(self.clone()).send().await },
            )
            .await?;
        Ok(())
    }
}
