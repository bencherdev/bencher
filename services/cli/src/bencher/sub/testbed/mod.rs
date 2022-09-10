use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::testbed::CliTestbed,
    CliError,
};

mod create;
mod list;
mod view;

#[derive(Debug)]
pub enum Testbed {
    List(list::List),
    Create(create::Create),
    View(view::View),
}

impl TryFrom<CliTestbed> for Testbed {
    type Error = CliError;

    fn try_from(testbed: CliTestbed) -> Result<Self, Self::Error> {
        Ok(match testbed {
            CliTestbed::List(list) => Self::List(list.try_into()?),
            CliTestbed::Create(create) => Self::Create(create.try_into()?),
            CliTestbed::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            Self::Create(create) => create.exec(wide).await,
            Self::View(view) => view.exec(wide).await,
        }
    }
}
