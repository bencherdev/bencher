use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::CliOrganizationList,
    CliError,
};

const ORGANIZATIONS_PATH: &str = "/v0/organizations";

#[derive(Debug)]
pub struct List {
    pub backend: Backend,
}

impl TryFrom<CliOrganizationList> for List {
    type Error = CliError;

    fn try_from(list: CliOrganizationList) -> Result<Self, Self::Error> {
        let CliOrganizationList { backend } = list;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend.get(ORGANIZATIONS_PATH).await?;
        Ok(())
    }
}
