use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

#[cfg(feature = "admin")]
mod admin;
#[cfg(feature = "plus")]
mod plus;
mod test;
#[cfg(feature = "api")]
mod types;
mod version;

#[cfg(feature = "admin")]
use admin::email_list::EmailList;
#[cfg(feature = "plus")]
use plus::{
    image::Image, index::Index, license::License, prompt::Prompt, stats::Stats,
    translate::Translate,
};
use test::{
    examples::Examples, netlify_test::NetlifyTest, seed_test::SeedTest, smoke_test::SmokeTest,
};
#[cfg(feature = "api")]
use types::{swagger::Swagger, types::Types, typeshare::Typeshare};
use version::Version;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Sub {
    Version(Version),
    #[cfg(feature = "api")]
    Typeshare(Typeshare),
    #[cfg(feature = "api")]
    Swagger(Swagger),
    #[cfg(feature = "api")]
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
    SeedTest(SeedTest),
    Examples(Examples),
    SmokeTest(SmokeTest),
    NetlifyTest(NetlifyTest),
    #[cfg(feature = "plus")]
    License(License),
    #[cfg(feature = "admin")]
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
            #[cfg(feature = "api")]
            TaskSub::Typeshare(typeshare) => Self::Typeshare(typeshare.try_into()?),
            #[cfg(feature = "api")]
            TaskSub::Swagger(swagger) => Self::Swagger(swagger.try_into()?),
            #[cfg(feature = "api")]
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
            TaskSub::SeedTest(seed_test) => Self::SeedTest(seed_test.try_into()?),
            TaskSub::Examples(examples) => Self::Examples(examples.try_into()?),
            TaskSub::SmokeTest(smoke_test) => Self::SmokeTest(smoke_test.try_into()?),
            TaskSub::NetlifyTest(netlify_test) => Self::NetlifyTest(netlify_test.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::License(license) => Self::License(license.try_into()?),
            #[cfg(feature = "admin")]
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
            #[cfg(feature = "api")]
            Self::Typeshare(typeshare) => typeshare.exec(),
            #[cfg(feature = "api")]
            Self::Swagger(swagger) => swagger.exec(),
            #[cfg(feature = "api")]
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
            Self::SeedTest(seed_test) => seed_test.exec(),
            Self::Examples(examples) => examples.exec(),
            Self::SmokeTest(smoke_test) => smoke_test.exec(),
            Self::NetlifyTest(netlify_test) => netlify_test.exec().await,
            #[cfg(feature = "plus")]
            Self::License(license) => license.exec(),
            #[cfg(feature = "admin")]
            Self::EmailList(email_list) => email_list.exec().await,
        }
    }
}
