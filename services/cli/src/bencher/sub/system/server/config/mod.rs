use crate::{bencher::sub::SubCmd, parser::system::server::CliConfig, CliError};

mod console;
mod update;
mod view;

#[derive(Debug)]
pub enum Config {
    View(view::View),
    Update(update::Update),
    Console(console::Console),
}

impl TryFrom<CliConfig> for Config {
    type Error = CliError;

    fn try_from(config: CliConfig) -> Result<Self, Self::Error> {
        Ok(match config {
            CliConfig::View(view) => Self::View(view.try_into()?),
            CliConfig::Update(update) => Self::Update(update.try_into()?),
            CliConfig::Console(console) => Self::Console(console.try_into()?),
        })
    }
}

impl SubCmd for Config {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Console(console) => console.exec().await,
        }
    }
}
