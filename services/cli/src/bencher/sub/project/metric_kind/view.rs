use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonMetricKind, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::metric_kind::CliMetricKindView,
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
        let _json: JsonMetricKind = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_metric_kind_get()
                        .project(self.project.clone())
                        .metric_kind(self.metric_kind.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
