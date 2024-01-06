use crate::{bencher::sub::SubCmd, parser::project::CliProject, CliError};

mod allowed;
mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Project {
    Create(create::Create),
    List(list::List),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
    Allowed(allowed::Allowed),
}

impl TryFrom<CliProject> for Project {
    type Error = CliError;

    fn try_from(project: CliProject) -> Result<Self, Self::Error> {
        Ok(match project {
            CliProject::Create(create) => Self::Create(create.try_into()?),
            CliProject::List(list) => Self::List(list.try_into()?),
            CliProject::View(view) => Self::View(view.try_into()?),
            CliProject::Update(update) => Self::Update(update.try_into()?),
            CliProject::Delete(delete) => Self::Delete(delete.try_into()?),
            CliProject::Allowed(allowed) => Self::Allowed(allowed.try_into()?),
        })
    }
}

impl SubCmd for Project {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Create(create) => create.exec().await,
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
            Self::Allowed(allowed) => allowed.exec().await,
        }
    }
}
