use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::project::measure::CliMeasure, CliError};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Measure {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliMeasure> for Measure {
    type Error = CliError;

    fn try_from(measure: CliMeasure) -> Result<Self, Self::Error> {
        Ok(match measure {
            CliMeasure::List(list) => Self::List(list.try_into()?),
            CliMeasure::Create(create) => Self::Create(create.try_into()?),
            CliMeasure::View(view) => Self::View(view.try_into()?),
            CliMeasure::Update(update) => Self::Update(update.try_into()?),
            CliMeasure::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Measure {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
