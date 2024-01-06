use crate::{bencher::sub::SubCmd, parser::project::alert::CliAlert, CliError};

mod list;
mod stats;
mod update;
mod view;

#[derive(Debug)]
pub enum Alert {
    List(list::List),
    View(view::View),
    Update(update::Update),
    Stats(stats::Stats),
}

impl TryFrom<CliAlert> for Alert {
    type Error = CliError;

    fn try_from(alert: CliAlert) -> Result<Self, Self::Error> {
        Ok(match alert {
            CliAlert::List(list) => Self::List(list.try_into()?),
            CliAlert::View(view) => Self::View(view.try_into()?),
            CliAlert::Update(update) => Self::Update(update.try_into()?),
            CliAlert::Stats(stats) => Self::Stats(stats.try_into()?),
        })
    }
}

impl SubCmd for Alert {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Stats(stats) => stats.exec().await,
        }
    }
}
