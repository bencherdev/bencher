use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

mod test_netlify;

use test_netlify::TestNetlify;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences)]
#[derive(Debug)]
pub enum Sub {
    Dev(TestNetlify),
    Prod(TestNetlify),
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
            TaskSub::Dev(test_netlify) => Self::Dev(TestNetlify::dev(test_netlify)),
            TaskSub::Prod(test_netlify) => Self::Prod(TestNetlify::prod(test_netlify)),
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
            Self::Dev(test_netlify) | Self::Prod(test_netlify) => test_netlify.exec().await,
        }
    }
}
