use clap::Parser;

use crate::parser::{CliSub, CliTask};

mod fly_test;
mod netlify_test;
mod release_notes;
mod swagger;
#[cfg(feature = "plus")]
mod translate;
mod types;
mod typeshare;

use fly_test::FlyTest;
use netlify_test::NetlifyTest;
use release_notes::ReleaseNotes;
use swagger::Swagger;
#[cfg(feature = "plus")]
use translate::Translate;
use types::Types;
use typeshare::Typeshare;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences)]
#[derive(Debug)]
pub enum Sub {
    Typeshare(Typeshare),
    Swagger(Swagger),
    Types(Types),
    #[cfg(feature = "plus")]
    Translate(Translate),
    FlyTest(FlyTest),
    NetlifyTest(NetlifyTest),
    ReleaseNotes(ReleaseNotes),
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
            CliSub::Typeshare(typeshare) => Self::Typeshare(typeshare.try_into()?),
            CliSub::Swagger(swagger) => Self::Swagger(swagger.try_into()?),
            CliSub::Types(types) => Self::Types(types.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Translate(translate) => Self::Translate(translate.try_into()?),
            CliSub::FlyTest(fly_test) => Self::FlyTest(fly_test.try_into()?),
            CliSub::NetlifyTest(netlify_test) => Self::NetlifyTest(netlify_test.try_into()?),
            CliSub::ReleaseNotes(release_notes) => Self::ReleaseNotes(release_notes.try_into()?),
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
            Self::Typeshare(typeshare) => typeshare.exec(),
            Self::Swagger(swagger) => swagger.exec(),
            Self::Types(types) => types.exec(),
            #[cfg(feature = "plus")]
            Self::Translate(translate) => translate.exec().await,
            Self::FlyTest(fly_test) => fly_test.exec(),
            Self::NetlifyTest(netlify_test) => netlify_test.exec().await,
            Self::ReleaseNotes(release_notes) => release_notes.exec(),
        }
    }
}
