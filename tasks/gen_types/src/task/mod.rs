use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

mod spec;
mod ts;
mod types;

use spec::Spec;
use ts::Ts;
use types::Types;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences)]
#[derive(Debug)]
pub enum Sub {
    Types(Types),
    Spec(Spec),
    Ts(Ts),
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
            TaskSub::Types(types) => Self::Types(types.try_into()?),
            TaskSub::Spec(spec) => Self::Spec(spec.try_into()?),
            TaskSub::Ts(ts) => Self::Ts(ts.try_into()?),
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
            Self::Types(types) => types.exec(),
            Self::Spec(spec) => spec.exec(),
            Self::Ts(ts) => ts.exec(),
        }
    }
}
