use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::admin::CliAdmin,
    CliError,
};

mod config;
mod restart;

#[derive(Debug)]
pub enum Admin {
    Restart(restart::Restart),
    Config(config::Config),
}

impl TryFrom<CliAdmin> for Admin {
    type Error = CliError;

    fn try_from(admin: CliAdmin) -> Result<Self, Self::Error> {
        Ok(match admin {
            CliAdmin::Restart(restart) => Self::Restart(restart.try_into()?),
            CliAdmin::Config(config) => Self::Config(config.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Admin {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::Restart(restart) => restart.exec(wide).await,
            Self::Config(config) => config.exec(wide).await,
        }
    }
}
