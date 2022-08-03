use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::branch::CliBranchList,
    BencherError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliBranchList> for List {
    type Error = BencherError;

    fn try_from(list: CliBranchList) -> Result<Self, Self::Error> {
        let CliBranchList { project, backend } = list;
        Ok(Self {
            project,
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!("/v0/projects/{}/branches", self.project.as_str()))
            .await?;
        Ok(())
    }
}
