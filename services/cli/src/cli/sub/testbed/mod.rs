use async_trait::async_trait;

use crate::{
    cli::{
        clap::CliTestbed,
        sub::SubCmd,
        wide::Wide,
    },
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
            CliTestbed::Create(create) => Self::Create(create.into()),
        })
    }
}

#[async_trait]
impl SubCmd for Testbed {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Create(testbed) => testbed.exec(wide).await,
        }
    }
}
