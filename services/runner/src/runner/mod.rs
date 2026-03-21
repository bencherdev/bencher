use clap::Parser as _;

use crate::error::RunnerCliError;
use crate::parser::{CliRunner, CliSub};

#[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
mod run;
#[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
mod up;

#[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
use run::Run;
#[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
use up::Up;

#[derive(Debug)]
pub struct Runner {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    Up(Up),
    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    Run(Run),
    #[cfg(not(all(feature = "plus", any(target_os = "linux", debug_assertions))))]
    Unsupported,
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

    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Up(up) => Self::Up(up.try_into()?),
            CliSub::Run(run) => Self::Run(run.try_into()?),
        })
    }

    #[cfg(not(all(feature = "plus", any(target_os = "linux", debug_assertions))))]
    fn try_from(_sub: CliSub) -> Result<Self, Self::Error> {
        Ok(Self::Unsupported)
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
            #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
            Self::Up(up) => up.exec(),
            #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
            Self::Run(run) => run.exec(),
            #[cfg(not(all(feature = "plus", any(target_os = "linux", debug_assertions))))]
            Self::Unsupported => Err(RunnerCliError::Unsupported),
        }
    }
}
