use clap::Parser;

use crate::parser::{CliSub, CliTask};

mod deb;
mod fly_test;
mod netlify_test;
mod notify;
#[cfg(feature = "plus")]
mod prompt;
mod release_notes;
#[cfg(feature = "plus")]
mod stats;
mod swagger;
mod template;
#[cfg(feature = "plus")]
mod translate;
mod types;
mod typeshare;

use deb::Deb;
use fly_test::FlyTest;
use netlify_test::NetlifyTest;
use notify::Notify;
#[cfg(feature = "plus")]
use prompt::Prompt;
use release_notes::ReleaseNotes;
#[cfg(feature = "plus")]
use stats::Stats;
use swagger::Swagger;
use template::Template;
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
    Template(Template),
    #[cfg(feature = "plus")]
    Stats(Stats),
    #[cfg(feature = "plus")]
    Prompt(Prompt),
    #[cfg(feature = "plus")]
    Translate(Translate),
    FlyTest(FlyTest),
    NetlifyTest(NetlifyTest),
    Deb(Deb),
    ReleaseNotes(ReleaseNotes),
    Notify(Notify),
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
            CliSub::Template(template) => Self::Template(template.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Stats(stats) => Self::Stats(stats.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Prompt(prompt) => Self::Prompt(prompt.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Translate(translate) => Self::Translate(translate.try_into()?),
            CliSub::FlyTest(fly_test) => Self::FlyTest(fly_test.try_into()?),
            CliSub::NetlifyTest(netlify_test) => Self::NetlifyTest(netlify_test.try_into()?),
            CliSub::Deb(deb) => Self::Deb(deb.try_into()?),
            CliSub::ReleaseNotes(release_notes) => Self::ReleaseNotes(release_notes.try_into()?),
            CliSub::Notify(notify) => Self::Notify(notify.try_into()?),
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
            Self::Template(template) => template.exec(),
            #[cfg(feature = "plus")]
            Self::Stats(stats) => stats.exec().await,
            #[cfg(feature = "plus")]
            Self::Prompt(prompt) => prompt.exec().await,
            #[cfg(feature = "plus")]
            Self::Translate(translate) => translate.exec().await,
            Self::FlyTest(fly_test) => fly_test.exec(),
            Self::NetlifyTest(netlify_test) => netlify_test.exec().await,
            Self::Deb(deb) => deb.exec(),
            Self::ReleaseNotes(release_notes) => release_notes.exec(),
            Self::Notify(notify) => notify.exec().await,
        }
    }
}
