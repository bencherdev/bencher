use clap::Parser as _;

use crate::parser::{TaskSub, TaskTask};

mod live;
mod plus;

use live::Live;
#[cfg(feature = "plus")]
use plus::{email_list::EmailList, index::Index, license::License, stats::Stats};

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[expect(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Sub {
    Live(Live),
    #[cfg(feature = "plus")]
    Index(Index),
    #[cfg(feature = "plus")]
    Stats(Stats),
    #[cfg(feature = "plus")]
    License(License),
    #[cfg(feature = "plus")]
    EmailList(EmailList),
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
            TaskSub::Live(live) => Self::Live(live.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Index(index) => Self::Index(index.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Stats(stats) => Self::Stats(stats.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::License(license) => Self::License(license.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::EmailList(email_list) => Self::EmailList(email_list.try_into()?),
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
            Self::Live(live) => live.exec().await,
            #[cfg(feature = "plus")]
            Self::Index(index) => index.exec().await,
            #[cfg(feature = "plus")]
            Self::Stats(stats) => stats.exec().await,
            #[cfg(feature = "plus")]
            Self::License(license) => license.exec(),
            #[cfg(feature = "plus")]
            Self::EmailList(email_list) => email_list.exec().await,
        }
    }
}
