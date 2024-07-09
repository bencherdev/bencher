use bencher_client::types::AlertStatus;

use crate::{
    bencher::sub::SubCmd,
    parser::project::alert::{CliAlert, CliAlertStatus},
    CliError,
};

mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Alert {
    List(list::List),
    View(view::View),
    Update(update::Update),
}

impl TryFrom<CliAlert> for Alert {
    type Error = CliError;

    fn try_from(alert: CliAlert) -> Result<Self, Self::Error> {
        Ok(match alert {
            CliAlert::List(list) => Self::List(list.try_into()?),
            CliAlert::View(view) => Self::View(view.try_into()?),
            CliAlert::Update(update) => Self::Update(update.try_into()?),
        })
    }
}

impl SubCmd for Alert {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
        }
    }
}

impl From<CliAlertStatus> for AlertStatus {
    fn from(status: CliAlertStatus) -> Self {
        match status {
            CliAlertStatus::Active => Self::Active,
            CliAlertStatus::Dismissed => Self::Dismissed,
        }
    }
}
