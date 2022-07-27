use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::wide::Wide,
    cli::CliSub,
    BencherError,
};

mod auth;
mod project;
mod run;
mod subcmd;
mod testbed;

use auth::Auth;
use project::Project;
use run::Run;
pub use subcmd::SubCmd;
use testbed::Testbed;

#[derive(Debug)]
pub enum Sub {
    Auth(Auth),
    Run(Run),
    Project(Project),
    Testbed(Testbed),
}

impl TryFrom<CliSub> for Sub {
    type Error = BencherError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Auth(auth) => Self::Auth(Auth::try_from(auth)?),
            CliSub::Run(run) => Self::Run(Run::try_from(run)?),
            CliSub::Project(project) => Self::Project(Project::try_from(project)?),
            CliSub::Testbed(testbed) => Self::Testbed(Testbed::try_from(testbed)?),
        })
    }
}

pub fn map_sub(sub: Option<CliSub>) -> Result<Option<Sub>, BencherError> {
    if let Some(sub) = sub {
        Ok(Some(Sub::try_from(sub)?))
    } else {
        Ok(None)
    }
}

#[async_trait]
impl SubCmd for Sub {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Auth(auth) => auth.exec(wide).await,
            Self::Run(run) => run.exec(wide).await,
            Self::Project(project) => project.exec(wide).await,
            Self::Testbed(testbed) => testbed.exec(wide).await,
        }
    }
}
