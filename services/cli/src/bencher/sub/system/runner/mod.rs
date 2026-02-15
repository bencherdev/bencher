use crate::{CliError, bencher::sub::SubCmd, parser::system::runner::CliRunner};

mod create;
mod list;
mod spec;
mod token;
mod update;
mod view;

#[derive(Debug)]
pub enum Runner {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Token(token::Token),
    Spec(spec::Spec),
}

impl TryFrom<CliRunner> for Runner {
    type Error = CliError;

    fn try_from(runner: CliRunner) -> Result<Self, Self::Error> {
        Ok(match runner {
            CliRunner::List(list) => Self::List(list.try_into()?),
            CliRunner::Create(create) => Self::Create(create.try_into()?),
            CliRunner::View(view) => Self::View(view.try_into()?),
            CliRunner::Update(update) => Self::Update(update.try_into()?),
            CliRunner::Token(token) => Self::Token(token.try_into()?),
            CliRunner::Spec(spec) => Self::Spec(spec.try_into()?),
        })
    }
}

impl SubCmd for Runner {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Token(token) => token.exec().await,
            Self::Spec(spec) => spec.exec().await,
        }
    }
}
