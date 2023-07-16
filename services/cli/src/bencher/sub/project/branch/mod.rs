use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::project::branch::CliBranch, CliError};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Branch {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliBranch> for Branch {
    type Error = CliError;

    fn try_from(branch: CliBranch) -> Result<Self, Self::Error> {
        Ok(match branch {
            CliBranch::List(list) => Self::List(list.try_into()?),
            CliBranch::Create(create) => Self::Create(create.try_into()?),
            CliBranch::View(view) => Self::View(view.try_into()?),
            CliBranch::Update(update) => Self::Update(update.try_into()?),
            CliBranch::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Branch {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
