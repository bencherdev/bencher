use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::testbed::CliTestbedList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliTestbedList> for List {
    type Error = CliError;

    fn try_from(list: CliTestbedList) -> Result<Self, Self::Error> {
        let CliTestbedList { project, backend } = list;
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
            .get(&format!("/v0/projects/{}/testbeds", self.project))
            .await?;
        Ok(())
    }
}
