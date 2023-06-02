use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::project::alert::CliAlert, CliError};

mod list;
mod view;

#[derive(Debug)]
pub enum Alert {
    List(list::List),
    View(view::View),
}

impl TryFrom<CliAlert> for Alert {
    type Error = CliError;

    fn try_from(alert: CliAlert) -> Result<Self, Self::Error> {
        Ok(match alert {
            CliAlert::List(list) => Self::List(list.try_into()?),
            CliAlert::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Alert {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
        }
    }
}
