use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::member::CliMemberList,
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub org: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMemberList> for List {
    type Error = CliError;

    fn try_from(list: CliMemberList) -> Result<Self, Self::Error> {
        let CliMemberList { org, backend } = list;
        Ok(Self {
            org,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/organizations/{}/members", self.org))
            .await?;
        Ok(())
    }
}
