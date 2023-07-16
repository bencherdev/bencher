use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::project::testbed::CliTestbed, CliError};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Testbed {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliTestbed> for Testbed {
    type Error = CliError;

    fn try_from(testbed: CliTestbed) -> Result<Self, Self::Error> {
        Ok(match testbed {
            CliTestbed::List(list) => Self::List(list.try_into()?),
            CliTestbed::Create(create) => Self::Create(create.try_into()?),
            CliTestbed::View(view) => Self::View(view.try_into()?),
            CliTestbed::Update(update) => Self::Update(update.try_into()?),
            CliTestbed::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
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
