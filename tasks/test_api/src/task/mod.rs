use clap::Parser as _;

use crate::parser::{TaskSub, TaskTask};

mod oci;
mod test;

use oci::Oci;
use test::{examples::Examples, seed_test::SeedTest, smoke_test::SmokeTest};

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    SeedTest(SeedTest),
    Examples(Examples),
    SmokeTest(SmokeTest),
    Oci(Oci),
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
            TaskSub::Seed(seed_test) => Self::SeedTest(seed_test.try_into()?),
            TaskSub::Examples(examples) => Self::Examples(examples.try_into()?),
            TaskSub::Smoke(smoke_test) => Self::SmokeTest(smoke_test.try_into()?),
            TaskSub::Oci(oci) => Self::Oci(oci.try_into()?),
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskTask::parse().try_into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::SeedTest(seed_test) => seed_test.exec(),
            Self::Examples(examples) => examples.exec(),
            Self::SmokeTest(smoke_test) => smoke_test.exec(),
            Self::Oci(oci) => oci.exec(),
        }
    }
}
