use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub org: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliProjectList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectList) -> Result<Self, Self::Error> {
        let CliProjectList { org, backend } = list;
        Ok(Self {
            org,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/organizations/{}/projects", self.org))
            .await?;
        Ok(())
    }
}
