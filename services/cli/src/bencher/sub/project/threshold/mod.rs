use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::project::threshold::CliThreshold, CliError};

mod create;
mod list;
mod statistic;
mod view;

#[derive(Debug)]
pub enum Threshold {
    List(list::List),
    Create(create::Create),
    View(view::View),
}

impl TryFrom<CliThreshold> for Threshold {
    type Error = CliError;

    fn try_from(threshold: CliThreshold) -> Result<Self, Self::Error> {
        Ok(match threshold {
            CliThreshold::List(list) => Self::List(list.try_into()?),
            CliThreshold::Create(create) => Self::Create(create.try_into()?),
            CliThreshold::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Threshold {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
        }
    }
}
