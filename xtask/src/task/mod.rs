use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

#[cfg(feature = "plus")]
mod plus;
mod types;
mod version;

#[cfg(feature = "plus")]
use plus::{
    email_list::EmailList, image::Image, index::Index, license::License, prompt::Prompt,
    stats::Stats, translate::Translate,
};
use types::Types;
use version::Version;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Sub {
    Version(Version),
    Types(Types),
    #[cfg(feature = "plus")]
    Index(Index),
    #[cfg(feature = "plus")]
    Stats(Stats),
    #[cfg(feature = "plus")]
    Prompt(Prompt),
    #[cfg(feature = "plus")]
    Translate(Translate),
    #[cfg(feature = "plus")]
    Image(Image),
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
            TaskSub::Version(version) => Self::Version(version.try_into()?),
            TaskSub::Types(types) => Self::Types(types.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Index(index) => Self::Index(index.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Stats(stats) => Self::Stats(stats.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Prompt(prompt) => Self::Prompt(prompt.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Translate(translate) => Self::Translate(translate.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Image(image) => Self::Image(image.try_into()?),
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
            Self::Version(version) => version.exec(),
            Self::Types(types) => types.exec(),
            #[cfg(feature = "plus")]
            Self::Index(index) => index.exec().await,
            #[cfg(feature = "plus")]
            Self::Stats(stats) => stats.exec().await,
            #[cfg(feature = "plus")]
            Self::Prompt(prompt) => prompt.exec().await,
            #[cfg(feature = "plus")]
            Self::Translate(translate) => translate.exec().await,
            #[cfg(feature = "plus")]
            Self::Image(image) => image.exec().await,
            #[cfg(feature = "plus")]
            Self::License(license) => license.exec(),
            #[cfg(feature = "plus")]
            Self::EmailList(email_list) => email_list.exec().await,
        }
    }
}
