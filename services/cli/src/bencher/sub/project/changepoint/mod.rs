#![cfg(feature = "plus")]

use crate::{CliError, bencher::sub::SubCmd, parser::project::changepoint::CliChangepoint};

mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Changepoint {
    List(list::List),
    View(view::View),
    Update(update::Update),
}

impl TryFrom<CliChangepoint> for Changepoint {
    type Error = CliError;

    fn try_from(changepoint: CliChangepoint) -> Result<Self, Self::Error> {
        Ok(match changepoint {
            CliChangepoint::List(list) => Self::List(list.try_into()?),
            CliChangepoint::View(view) => Self::View(view.try_into()?),
            CliChangepoint::Update(update) => Self::Update(update.try_into()?),
        })
    }
}

impl SubCmd for Changepoint {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
        }
    }
}
