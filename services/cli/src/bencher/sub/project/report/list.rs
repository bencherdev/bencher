use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::report::CliReportList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliReportList> for List {
    type Error = CliError;

    fn try_from(list: CliReportList) -> Result<Self, Self::Error> {
        let CliReportList { project, backend } = list;
        Ok(Self {
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/projects/{}/reports", self.project))
            .await?;
        Ok(())
    }
}
