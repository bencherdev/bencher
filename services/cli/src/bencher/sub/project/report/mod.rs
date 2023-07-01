use async_trait::async_trait;

use super::run::Run as Create;
use crate::{bencher::sub::SubCmd, cli::project::report::CliReport, CliError};

mod delete;
mod list;
mod view;

#[derive(Debug)]
pub enum Report {
    List(list::List),
    Create(Box<Create>),
    View(view::View),
    Delete(delete::Delete),
}

impl TryFrom<CliReport> for Report {
    type Error = CliError;

    fn try_from(report: CliReport) -> Result<Self, Self::Error> {
        Ok(match report {
            CliReport::List(list) => Self::List(list.try_into()?),
            CliReport::Create(create) => Self::Create(Box::new((*create).try_into()?)),
            CliReport::View(view) => Self::View(view.try_into()?),
            CliReport::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Report {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(create) => create.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
