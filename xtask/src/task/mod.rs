use clap::Parser;

use crate::cli::{CliSub, CliTask};

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    Fmt,
}

impl TryFrom<CliTask> for Task {
    type Error = anyhow::Error;

    fn try_from(task: CliTask) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<CliSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Fmt => Self::Fmt,
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        CliTask::parse().try_into()
    }

    pub async fn exec(&self) -> anyhow::Result<()> {
        self.sub.exec().await
    }
}

impl Sub {
    pub async fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Fmt => Ok(()),
        }
    }
}
