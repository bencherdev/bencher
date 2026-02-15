#![cfg(feature = "plus")]

use crate::{CliError, bencher::sub::SubCmd, parser::project::job::CliJob};

mod list;
mod view;

#[derive(Debug)]
pub enum Job {
    List(list::List),
    View(view::View),
}

impl TryFrom<CliJob> for Job {
    type Error = CliError;

    fn try_from(job: CliJob) -> Result<Self, Self::Error> {
        Ok(match job {
            CliJob::List(list) => Self::List(list.try_into()?),
            CliJob::View(view) => Self::View(view.try_into()?),
        })
    }
}

impl SubCmd for Job {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::View(view) => view.exec().await,
        }
    }
}
