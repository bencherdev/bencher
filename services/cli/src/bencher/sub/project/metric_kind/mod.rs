use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::project::metric_kind::CliMetricKind, CliError};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum MetricKind {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliMetricKind> for MetricKind {
    type Error = CliError;

    fn try_from(metric_kind: CliMetricKind) -> Result<Self, Self::Error> {
        Ok(match metric_kind {
            CliMetricKind::List(list) => Self::List(list.try_into()?),
            CliMetricKind::Create(create) => Self::Create(create.try_into()?),
            CliMetricKind::View(view) => Self::View(view.try_into()?),
            CliMetricKind::Update(update) => Self::Update(update.try_into()?),
            CliMetricKind::Delete(delete) => Self::Delete(delete.try_into()?),
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
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
