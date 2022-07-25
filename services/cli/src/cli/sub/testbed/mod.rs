use async_trait::async_trait;

use crate::{
    cli::{
        sub::SubCmd,
        wide::Wide,
    },
    cmd::CliTestbed,
    BencherError,
};

mod create;

#[derive(Debug)]
pub enum Testbed {
    Create(create::Testbed),
}

impl TryFrom<CliTestbed> for Testbed {
    type Error = BencherError;

    fn try_from(testbed: CliTestbed) -> Result<Self, Self::Error> {
        Ok(match testbed {
            CliTestbed::Create(create) => Self::Create(create.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Create(create) => create.exec(wide).await,
        }
    }
}
