use clap::Parser as _;

use crate::error::RunnerCliError;
use crate::parser::{CliRunner, CliSub};

#[cfg(feature = "plus")]
mod run;
#[cfg(feature = "plus")]
mod up;

#[cfg(feature = "plus")]
use run::Run;
#[cfg(feature = "plus")]
use up::Up;

#[derive(Debug)]
pub struct Runner {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    #[cfg(feature = "plus")]
    Up(Up),
    #[cfg(feature = "plus")]
    Run(Run),
}

impl TryFrom<CliRunner> for Runner {
    type Error = RunnerCliError;

    fn try_from(cli: CliRunner) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: cli.sub.try_into()?,
        })
    }
}

impl TryFrom<CliSub> for Sub {
    type Error = RunnerCliError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            #[cfg(feature = "plus")]
            CliSub::Up(up) => Self::Up(up.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Run(run) => Self::Run(run.try_into()?),
        })
    }
}

impl Runner {
    pub fn new() -> Result<Self, RunnerCliError> {
        CliRunner::parse().try_into()
    }

    pub fn exec(self) -> Result<(), RunnerCliError> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(self) -> Result<(), RunnerCliError> {
        match self {
            #[cfg(feature = "plus")]
            Self::Up(up) => up.exec(),
            #[cfg(feature = "plus")]
            Self::Run(run) => run.exec(),
        }
    }
}
