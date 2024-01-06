use crate::{bencher::sub::SubCmd, parser::user::token::CliToken, CliError};

mod create;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Token {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
}

impl TryFrom<CliToken> for Token {
    type Error = CliError;

    fn try_from(token: CliToken) -> Result<Self, Self::Error> {
        Ok(match token {
            CliToken::List(list) => Self::List(list.try_into()?),
            CliToken::Create(create) => Self::Create(create.try_into()?),
            CliToken::View(view) => Self::View(view.try_into()?),
            CliToken::Update(update) => Self::Update(update.try_into()?),
        })
    }
}

impl SubCmd for Token {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
        }
    }
}
