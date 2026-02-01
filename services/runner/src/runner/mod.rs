use clap::Parser as _;

use crate::parser::{TaskRunner, TaskSub};

#[cfg(all(feature = "plus", target_os = "linux"))]
mod run;
#[cfg(all(feature = "plus", target_os = "linux"))]
mod vmm;

#[cfg(all(feature = "plus", target_os = "linux"))]
use run::Run;
#[cfg(all(feature = "plus", target_os = "linux"))]
use vmm::Vmm;

#[derive(Debug)]
pub struct Runner {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    #[cfg(all(feature = "plus", target_os = "linux"))]
    Run(Run),
    #[cfg(all(feature = "plus", target_os = "linux"))]
    Vmm(Vmm),
    #[cfg(not(all(feature = "plus", target_os = "linux")))]
    Unsupported,
}

impl TryFrom<TaskRunner> for Runner {
    type Error = anyhow::Error;

    fn try_from(task: TaskRunner) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    #[cfg(all(feature = "plus", target_os = "linux"))]
    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Run(run) => Self::Run(run.try_into()?),
            TaskSub::Vmm(vmm) => Self::Vmm(vmm.try_into()?),
        })
    }

    #[cfg(not(all(feature = "plus", target_os = "linux")))]
    fn try_from(_sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(Self::Unsupported)
    }
}

impl Runner {
    pub fn new() -> anyhow::Result<Self> {
        TaskRunner::parse().try_into()
    }

    pub fn exec(self) -> anyhow::Result<()> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(self) -> anyhow::Result<()> {
        match self {
            #[cfg(all(feature = "plus", target_os = "linux"))]
            Self::Run(run) => run.exec(),
            #[cfg(all(feature = "plus", target_os = "linux"))]
            Self::Vmm(vmm) => vmm.exec(),
            #[cfg(not(all(feature = "plus", target_os = "linux")))]
            Self::Unsupported => {
                anyhow::bail!("bencher-runner requires Linux with the `plus` feature")
            }
        }
    }
}
