#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::CliOrganizationUsage,
    CliError,
};

#[derive(Debug)]
pub struct Usage {
    pub organization: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationUsage> for Usage {
    type Error = CliError;

    fn try_from(usage: CliOrganizationUsage) -> Result<Self, Self::Error> {
        let CliOrganizationUsage {
            organization,
            backend,
        } = usage;
        Ok(Self {
            organization,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Usage {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/organizations/{}/usage", self.organization))
            .await?;
        Ok(())
    }
}
