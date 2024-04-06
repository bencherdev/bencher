use crate::{bencher::sub::SubCmd, parser::user::CliUser, CliError};

mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum User {
    List(list::List),
    View(view::View),
    Update(update::Update),
}

impl TryFrom<CliUser> for User {
    type Error = CliError;

    fn try_from(user: CliUser) -> Result<Self, Self::Error> {
        Ok(match user {
            CliUser::List(list) => Self::List(list.try_into()?),
            CliUser::View(view) => Self::View(view.try_into()?),
            CliUser::Update(update) => Self::Update(update.try_into()?),
        })
    }
}

impl SubCmd for User {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
        }
    }
}
