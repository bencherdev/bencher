use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonEmpty;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::admin::CliAdminRestart,
    CliError,
};

const RESTART_PATH: &str = "/v0/admin/restart";

#[derive(Debug, Clone)]
pub struct Restart {
    pub backend: Backend,
}

impl TryFrom<CliAdminRestart> for Restart {
    type Error = CliError;

    fn try_from(create: CliAdminRestart) -> Result<Self, Self::Error> {
        let CliAdminRestart { backend } = create;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Restart {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend.post(RESTART_PATH, &JsonEmpty {}).await?;
        Ok(())
    }
}
