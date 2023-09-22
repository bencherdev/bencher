use clap::Parser;

use crate::parser::{CliSub, CliTask};

mod release_notes;
mod swagger;
mod typeshare;

use release_notes::ReleaseNotes;
use swagger::Swagger;
use typeshare::Typeshare;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    Fmt,
    ReleaseNotes(ReleaseNotes),
    Swagger(Swagger),
    Typeshare(Typeshare),
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
            CliSub::ReleaseNotes(release_notes) => Self::ReleaseNotes(release_notes.try_into()?),
            CliSub::Swagger(swagger) => Self::Swagger(swagger.try_into()?),
            CliSub::Typeshare(typeshare) => Self::Typeshare(typeshare.try_into()?),
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
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Fmt => Ok(()),
            Self::ReleaseNotes(release_notes) => release_notes.exec(),
            Self::Swagger(swagger) => swagger.exec(),
            Self::Typeshare(typeshare) => typeshare.exec(),
        }
    }
}
