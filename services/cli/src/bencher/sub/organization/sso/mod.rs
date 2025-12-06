#![cfg(feature = "plus")]

use crate::{CliError, bencher::sub::SubCmd, parser::organization::sso::CliSso};

mod create;
mod delete;
mod list;
mod view;

#[derive(Debug)]
pub enum Sso {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Delete(delete::Delete),
}

impl TryFrom<CliSso> for Sso {
    type Error = CliError;

    fn try_from(sso: CliSso) -> Result<Self, Self::Error> {
        Ok(match sso {
            CliSso::List(list) => Self::List(list.try_into()?),
            CliSso::Create(create) => Self::Create(create.try_into()?),
            CliSso::View(view) => Self::View(view.try_into()?),
            CliSso::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

impl SubCmd for Sso {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
