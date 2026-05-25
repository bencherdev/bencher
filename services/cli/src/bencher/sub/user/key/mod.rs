use crate::{CliError, bencher::sub::SubCmd, parser::user::key::CliUserKey};

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

impl TryFrom<CliUserKey> for Key {
    type Error = CliError;

    fn try_from(key: CliUserKey) -> Result<Self, Self::Error> {
        Ok(match key {
            CliUserKey::List(list) => Self::List(list.try_into()?),
            CliUserKey::Create(create) => Self::Create(create.try_into()?),
            CliUserKey::View(view) => Self::View(view.try_into()?),
            CliUserKey::Update(update) => Self::Update(update.try_into()?),
            CliUserKey::Revoke(revoke) => Self::Revoke(revoke.try_into()?),
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
