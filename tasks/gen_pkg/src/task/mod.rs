use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

mod package;

use package::{deb::Deb, man::Man};

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences)]
#[derive(Debug)]
pub enum Sub {
    Man(Man),
    Deb(Deb),
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
            TaskSub::Man(man) => Self::Man(man.into()),
            TaskSub::Deb(deb) => Self::Deb(deb.try_into()?),
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
            Self::Man(man) => man.exec(),
            Self::Deb(deb) => deb.exec(),
        }
    }
}
