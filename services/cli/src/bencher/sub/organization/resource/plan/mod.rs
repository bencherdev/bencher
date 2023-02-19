use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::organization::plan::CliOrganizationPlan, CliError};

mod create;

#[derive(Debug)]
pub enum Plan {
    Create(create::Create),
}

impl TryFrom<CliOrganizationPlan> for Plan {
    type Error = CliError;

    fn try_from(plan: CliOrganizationPlan) -> Result<Self, Self::Error> {
        Ok(match plan {
            CliOrganizationPlan::Create(create) => Self::Create(create.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Plan {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Create(create) => create.exec().await,
        }
    }
}
