#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::CliOrganizationEntitlements,
    CliError,
};

#[derive(Debug)]
pub struct Entitlements {
    pub organization: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationEntitlements> for Entitlements {
    type Error = CliError;

    fn try_from(entitlements: CliOrganizationEntitlements) -> Result<Self, Self::Error> {
        let CliOrganizationEntitlements {
            organization,
            backend,
        } = entitlements;
        Ok(Self {
            organization,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Entitlements {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/organizations/{}/entitlements",
                self.organization
            ))
            .await?;
        Ok(())
    }
}
