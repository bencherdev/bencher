#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::usage::CliOrganizationUsage,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Usage {
    pub organization: ResourceId,
    pub backend: AuthBackend,
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
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_usage_get()
                    .organization(self.organization.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
