use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::project::CliProject, CliError};

mod create;
mod list;
mod view;

#[derive(Debug)]
pub enum Project {
    Create(create::Create),
    List(list::List),
    View(view::View),
}

impl TryFrom<CliProject> for Project {
    type Error = CliError;

    fn try_from(project: CliProject) -> Result<Self, Self::Error> {
        Ok(match project {
            CliProject::Create(create) => Self::Create(create.try_into()?),
            CliProject::List(list) => Self::List(list.try_into()?),
            CliProject::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Project {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Create(create) => create.exec().await,
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
        }
    }
}
