use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, cli::project::metric_kind::CliMetricKind, CliError};

mod create;
mod list;
mod view;

#[derive(Debug)]
pub enum MetricKind {
    List(list::List),
    Create(create::Create),
    View(view::View),
}

impl TryFrom<CliMetricKind> for MetricKind {
    type Error = CliError;

    fn try_from(metric_kind: CliMetricKind) -> Result<Self, Self::Error> {
        Ok(match metric_kind {
            CliMetricKind::List(list) => Self::List(list.try_into()?),
            CliMetricKind::Create(create) => Self::Create(create.try_into()?),
            CliMetricKind::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for MetricKind {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
        }
    }
}
