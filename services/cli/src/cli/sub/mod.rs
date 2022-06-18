use std::convert::TryFrom;

use async_trait::async_trait;

use crate::cli::clap::CliSub;
use crate::cli::wide::Wide;
use crate::BencherError;

mod run;
mod subcmd;

use run::Run;
pub use subcmd::SubCmd;

#[derive(Debug)]
pub enum Sub {
    Run(Run),
}

impl TryFrom<CliSub> for Sub {
    type Error = BencherError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Run(run) => Sub::Run(Run::try_from(run)?),
            CliSub::Testbed(testbed) => todo!("Handle testbed subcommand: {testbed:?}"),
        })
    }
}

pub fn map_sub(sub: Option<CliSub>) -> Result<Option<Sub>, BencherError> {
    if let Some(sub) = sub {
        Ok(Some(Sub::try_from(sub)?))
    } else {
        Ok(None)
    }
}

#[async_trait]
impl SubCmd for Sub {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Sub::Run(run) => run.exec(wide).await,
        }
    }
}
