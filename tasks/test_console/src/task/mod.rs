use clap::Parser as _;

use crate::parser::{TaskSub, TaskTask};

#[cfg(feature = "plus")]
mod e2e;
mod test_console;

#[cfg(feature = "plus")]
use e2e::E2e;
use test_console::TestConsole;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    Dev(TestConsole),
    Prod(TestConsole),
    #[cfg(feature = "plus")]
    E2e(E2e),
}

impl TryFrom<TaskTask> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskTask) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Dev(test_console) => Self::Dev(TestConsole::dev(test_console)),
            TaskSub::Prod(test_console) => Self::Prod(TestConsole::prod(test_console)),
            #[cfg(feature = "plus")]
            TaskSub::E2e(e2e) => Self::E2e(e2e.into()),
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskTask::parse().try_into()
    }

    pub async fn exec(&self) -> anyhow::Result<()> {
        self.sub.exec().await
    }
}

impl Sub {
    pub async fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Dev(test_console) | Self::Prod(test_console) => test_console.exec().await,
            #[cfg(feature = "plus")]
            Self::E2e(e2e) => e2e.exec().await,
        }
    }
}
