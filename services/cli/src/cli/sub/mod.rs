use std::convert::TryFrom;

mod run;

use crate::cli::clap::CliSub;
use crate::cli::wide::Wide;
use crate::BencherError;
use run::Run;

#[derive(Debug)]
pub enum Sub {
    Run(Run),
}

impl TryFrom<CliSub> for Sub {
    type Error = BencherError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Run(run) => Sub::Run(Run::try_from(run)?),
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

impl Sub {
    pub async fn run(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Sub::Run(run) => run.run(wide).await,
        }
    }
}
