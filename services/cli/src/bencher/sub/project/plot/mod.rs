use crate::{CliError, bencher::sub::SubCmd, parser::project::plot::CliPlot};

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

#[derive(Debug, thiserror::Error)]
pub enum PlotError {
    #[error("Failed to send the plot parameters: {0}")]
    BadParameters(serde_json::Error),
    #[error("`--metric _` cannot be given with a metric name: `_` alone draws every metric again")]
    MetricBesideUnderscore,
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

fn metric_filter(metrics: Vec<bencher_json::MetricName>) -> bencher_client::types::MetricFilter {
    bencher_client::types::MetricFilter(metrics.into_iter().map(Into::into).collect())
}
