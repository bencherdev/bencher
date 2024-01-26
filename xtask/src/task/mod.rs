use clap::Parser;

use crate::parser::{TaskSub, TaskTask};

mod notify;
mod package;
#[cfg(feature = "plus")]
mod plus;
mod release_notes;
mod template;
mod test;
mod types;
mod version;

use notify::Notify;
use package::deb::Deb;
use package::man::Man;
#[cfg(feature = "plus")]
use plus::{index::Index, license::License, prompt::Prompt, stats::Stats, translate::Translate};
use release_notes::ReleaseNotes;
use template::Template;
use test::{
    examples::Examples, netlify_test::NetlifyTest, seed_test::SeedTest, smoke_test::SmokeTest,
};
use types::{swagger::Swagger, types::Types, typeshare::Typeshare};
use version::Version;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[allow(variant_size_differences)]
#[derive(Debug)]
pub enum Sub {
    Version(Version),
    Typeshare(Typeshare),
    Swagger(Swagger),
    Types(Types),
    Template(Template),
    #[cfg(feature = "plus")]
    Index(Index),
    #[cfg(feature = "plus")]
    Stats(Stats),
    #[cfg(feature = "plus")]
    Prompt(Prompt),
    #[cfg(feature = "plus")]
    Translate(Translate),
    SeedTest(SeedTest),
    Examples(Examples),
    SmokeTest(SmokeTest),
    NetlifyTest(NetlifyTest),
    Man(Man),
    Deb(Deb),
    ReleaseNotes(ReleaseNotes),
    Notify(Notify),
    #[cfg(feature = "plus")]
    License(License),
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
            TaskSub::Typeshare(typeshare) => Self::Typeshare(typeshare.try_into()?),
            TaskSub::Swagger(swagger) => Self::Swagger(swagger.try_into()?),
            TaskSub::Types(types) => Self::Types(types.try_into()?),
            TaskSub::Template(template) => Self::Template(template.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Index(index) => Self::Index(index.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Stats(stats) => Self::Stats(stats.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Prompt(prompt) => Self::Prompt(prompt.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Translate(translate) => Self::Translate(translate.try_into()?),
            TaskSub::SeedTest(seed_test) => Self::SeedTest(seed_test.try_into()?),
            TaskSub::Examples(examples) => Self::Examples(examples.try_into()?),
            TaskSub::SmokeTest(smoke_test) => Self::SmokeTest(smoke_test.try_into()?),
            TaskSub::NetlifyTest(netlify_test) => Self::NetlifyTest(netlify_test.try_into()?),
            TaskSub::Man(man) => Self::Man(man.into()),
            TaskSub::Deb(deb) => Self::Deb(deb.try_into()?),
            TaskSub::ReleaseNotes(release_notes) => Self::ReleaseNotes(release_notes.try_into()?),
            TaskSub::Notify(notify) => Self::Notify(notify.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::License(license) => Self::License(license.try_into()?),
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
            Self::Typeshare(typeshare) => typeshare.exec(),
            Self::Swagger(swagger) => swagger.exec(),
            Self::Types(types) => types.exec(),
            Self::Template(template) => template.exec(),
            #[cfg(feature = "plus")]
            Self::Index(index) => index.exec().await,
            #[cfg(feature = "plus")]
            Self::Stats(stats) => stats.exec().await,
            #[cfg(feature = "plus")]
            Self::Prompt(prompt) => prompt.exec().await,
            #[cfg(feature = "plus")]
            Self::Translate(translate) => translate.exec().await,
            Self::SeedTest(seed_test) => seed_test.exec(),
            Self::Examples(examples) => examples.exec(),
            Self::SmokeTest(smoke_test) => smoke_test.exec(),
            Self::NetlifyTest(netlify_test) => netlify_test.exec().await,
            Self::Man(man) => man.exec(),
            Self::Deb(deb) => deb.exec(),
            Self::ReleaseNotes(release_notes) => release_notes.exec(),
            Self::Notify(notify) => notify.exec().await,
            #[cfg(feature = "plus")]
            Self::License(license) => license.exec(),
        }
    }
}
