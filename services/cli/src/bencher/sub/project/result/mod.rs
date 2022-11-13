use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::project::result::CliResult, CliError};

mod view;

#[derive(Debug)]
pub enum Resultant {
    View(view::View),
}

impl TryFrom<CliResult> for Resultant {
    type Error = CliError;

    fn try_from(result: CliResult) -> Result<Self, Self::Error> {
        Ok(match result {
            CliResult::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Resultant {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::View(view) => view.exec().await,
        }
    }
}
