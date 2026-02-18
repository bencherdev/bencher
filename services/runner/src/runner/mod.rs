use clap::Parser as _;

use crate::error::RunnerCliError;
use crate::parser::{TaskRunner, TaskSub};

#[cfg(all(feature = "plus", target_os = "linux"))]
mod run;
#[cfg(all(feature = "plus", target_os = "linux"))]
mod up;

#[cfg(all(feature = "plus", target_os = "linux"))]
use run::Run;
#[cfg(all(feature = "plus", target_os = "linux"))]
use up::UpRunner;

#[derive(Debug)]
pub struct Runner {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    #[cfg(all(feature = "plus", target_os = "linux"))]
    Up(UpRunner),
    #[cfg(all(feature = "plus", target_os = "linux"))]
    Run(Run),
    #[cfg(not(all(feature = "plus", target_os = "linux")))]
    Unsupported,
}

impl TryFrom<TaskRunner> for Runner {
    type Error = RunnerCliError;

    fn try_from(task: TaskRunner) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = RunnerCliError;

    #[cfg(all(feature = "plus", target_os = "linux"))]
    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Up(up) => Self::Up(up.try_into()?),
            TaskSub::Run(run) => Self::Run(run.try_into()?),
        })
    }

    #[cfg(not(all(feature = "plus", target_os = "linux")))]
    fn try_from(_sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(Self::Unsupported)
    }
}

impl Runner {
    pub fn new() -> Result<Self, RunnerCliError> {
        TaskRunner::parse().try_into()
    }

    pub fn exec(self) -> Result<(), RunnerCliError> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(self) -> Result<(), RunnerCliError> {
        match self {
            #[cfg(all(feature = "plus", target_os = "linux"))]
            Self::Up(up) => up.exec(),
            #[cfg(all(feature = "plus", target_os = "linux"))]
            Self::Run(run) => run.exec(),
            #[cfg(not(all(feature = "plus", target_os = "linux")))]
            Self::Unsupported => Err(RunnerCliError::Unsupported),
        }
    }
}
