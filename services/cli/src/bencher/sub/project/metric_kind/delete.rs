use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::metric_kind::CliMetricKindDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMetricKindDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliMetricKindDelete) -> Result<Self, Self::Error> {
        let CliMetricKindDelete {
            project,
            metric_kind,
            backend,
        } = delete;
        Ok(Self {
            project,
            metric_kind,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_metric_kind_delete()
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
