use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::admin::CliConfig,
    CliError,
};

mod update;
mod view;

#[derive(Debug)]
pub enum Config {
    Update(update::Update),
    View(view::View),
}

impl TryFrom<CliConfig> for Config {
    type Error = CliError;

    fn try_from(config: CliConfig) -> Result<Self, Self::Error> {
        Ok(match config {
            CliConfig::Update(update) => Self::Update(update.try_into()?),
            CliConfig::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Config {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::Update(update) => update.exec(wide).await,
            Self::View(view) => view.exec(wide).await,
        }
    }
}
