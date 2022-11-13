use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::metric_kind::CliMetricKindList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMetricKindList> for List {
    type Error = CliError;

    fn try_from(list: CliMetricKindList) -> Result<Self, Self::Error> {
        let CliMetricKindList { project, backend } = list;
        Ok(Self {
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/projects/{}/metric-kinds", self.project))
            .await?;
        Ok(())
    }
}
