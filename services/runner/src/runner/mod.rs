#[cfg(feature = "plus")]
mod run;
#[cfg(feature = "plus")]
mod up;

#[cfg(feature = "plus")]
use clap::Parser as _;
#[cfg(feature = "plus")]
use run::Run;
#[cfg(feature = "plus")]
use up::Up;

#[cfg(feature = "plus")]
use crate::error::RunnerCliError;
#[cfg(feature = "plus")]
use crate::parser::{CliRunner, CliSub};

#[cfg(feature = "plus")]
#[derive(Debug)]
pub struct Runner {
    sub: Sub,
}

#[cfg(feature = "plus")]
#[derive(Debug)]
enum Sub {
    Up(Up),
    Run(Run),
}

#[cfg(feature = "plus")]
impl TryFrom<CliRunner> for Runner {
    type Error = RunnerCliError;

    fn try_from(cli: CliRunner) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: cli.sub.try_into()?,
        })
    }
}

#[cfg(feature = "plus")]
impl TryFrom<CliSub> for Sub {
    type Error = RunnerCliError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Up(up) => Self::Up(up.try_into()?),
            CliSub::Run(run) => Self::Run(run.try_into()?),
        })
    }
}

#[cfg(feature = "plus")]
impl Runner {
    pub fn new() -> Result<Self, RunnerCliError> {
        CliRunner::parse().try_into()
    }

    pub fn exec(self) -> Result<(), RunnerCliError> {
        self.sub.exec()
    }
}

#[cfg(feature = "plus")]
impl Sub {
    fn exec(self) -> Result<(), RunnerCliError> {
        match self {
            Self::Up(up) => up.exec(),
            Self::Run(run) => run.exec(),
        }
    }
}
