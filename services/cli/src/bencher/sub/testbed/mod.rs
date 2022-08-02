use async_trait::async_trait;

use crate::{
    bencher::{
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliTestbed,
    BencherError,
};

mod create;
mod list;

#[derive(Debug)]
pub enum Testbed {
    List(list::List),
    Create(create::Testbed),
}

impl TryFrom<CliTestbed> for Testbed {
    type Error = BencherError;

    fn try_from(testbed: CliTestbed) -> Result<Self, Self::Error> {
        Ok(match testbed {
            CliTestbed::List(list) => Self::List(list.try_into()?),
            CliTestbed::Create(create) => Self::Create(create.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            Self::Create(create) => create.exec(wide).await,
        }
    }
}
