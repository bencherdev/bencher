use crate::{bencher::sub::SubCmd, parser::user::CliUser, CliError};

mod view;

#[derive(Debug)]
pub enum User {
    View(view::View),
}

impl TryFrom<CliUser> for User {
    type Error = CliError;

    fn try_from(user: CliUser) -> Result<Self, Self::Error> {
        Ok(match user {
            CliUser::View(view) => Self::View(view.try_into()?),
        })
    }
}

impl SubCmd for User {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::View(view) => view.exec().await,
        }
    }
}
