use crate::{CliError, bencher::sub::SubCmd, parser::project::key::CliProjectKey};

mod create;
mod list;
mod revoke;
mod update;
mod view;

#[derive(Debug)]
pub enum Key {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Revoke(revoke::Revoke),
}

impl TryFrom<CliProjectKey> for Key {
    type Error = CliError;

    fn try_from(key: CliProjectKey) -> Result<Self, Self::Error> {
        Ok(match key {
            CliProjectKey::List(list) => Self::List(list.try_into()?),
            CliProjectKey::Create(create) => Self::Create(create.try_into()?),
            CliProjectKey::View(view) => Self::View(view.try_into()?),
            CliProjectKey::Update(update) => Self::Update(update.try_into()?),
            CliProjectKey::Revoke(revoke) => Self::Revoke(revoke.try_into()?),
        })
    }
}

impl SubCmd for Key {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Revoke(revoke) => revoke.exec().await,
        }
    }
}
