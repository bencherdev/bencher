use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::threshold::CliThresholdList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliThresholdList> for List {
    type Error = CliError;

    fn try_from(list: CliThresholdList) -> Result<Self, Self::Error> {
        let CliThresholdList { project, backend } = list;
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
            .get(&format!("/v0/projects/{}/thresholds", self.project))
            .await?;
        Ok(())
    }
}
