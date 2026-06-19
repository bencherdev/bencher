#![cfg(feature = "plus")]

use crate::{CliError, bencher::sub::SubCmd, parser::organization::plan::CliOrganizationPlan};

mod create;
mod delete;
mod update;
mod view;

#[derive(Debug)]
pub enum Plan {
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliOrganizationPlan> for Plan {
    type Error = CliError;

    fn try_from(plan: CliOrganizationPlan) -> Result<Self, Self::Error> {
        Ok(match plan {
            CliOrganizationPlan::Create(create) => Self::Create(create.try_into()?),
            CliOrganizationPlan::View(view) => Self::View(view.try_into()?),
            CliOrganizationPlan::Update(update) => Self::Update(update.try_into()?),
            CliOrganizationPlan::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

impl SubCmd for Plan {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
