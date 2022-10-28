use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::user::token::CliToken,
    CliError,
};

mod create;
mod list;
mod view;

#[derive(Debug)]
pub enum Token {
    List(list::List),
    Create(create::Create),
    View(view::View),
}

impl TryFrom<CliToken> for Token {
    type Error = CliError;

    fn try_from(token: CliToken) -> Result<Self, Self::Error> {
        Ok(match token {
            CliToken::List(list) => Self::List(list.try_into()?),
            CliToken::Create(create) => Self::Create(create.try_into()?),
            CliToken::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Token {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            Self::Create(create) => create.exec(wide).await,
            Self::View(view) => view.exec(wide).await,
        }
    }
}
