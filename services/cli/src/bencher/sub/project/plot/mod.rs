use crate::{bencher::sub::SubCmd, parser::project::plot::CliPlot, CliError};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Plot {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliPlot> for Plot {
    type Error = CliError;

    fn try_from(plot: CliPlot) -> Result<Self, Self::Error> {
        Ok(match plot {
            CliPlot::List(list) => Self::List(list.try_into()?),
            CliPlot::Create(create) => Self::Create(create.try_into()?),
            CliPlot::View(view) => Self::View(view.try_into()?),
            CliPlot::Update(update) => Self::Update(update.try_into()?),
            CliPlot::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

impl SubCmd for Plot {
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
