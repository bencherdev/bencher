use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::metric_kind::CliMetricKindView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMetricKindView> for View {
    type Error = CliError;

    fn try_from(view: CliMetricKindView) -> Result<Self, Self::Error> {
        let CliMetricKindView {
            project,
            metric_kind,
            backend,
        } = view;
        Ok(Self {
            project,
            metric_kind,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/metric-kinds/{}",
                self.project, self.metric_kind
            ))
            .await?;
        Ok(())
    }
}
