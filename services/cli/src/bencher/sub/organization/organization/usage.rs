#![cfg(feature = "plus")]

use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::usage::CliOrganizationUsage,
};

#[derive(Debug, Clone)]
pub struct Usage {
    pub organization: OrganizationResourceId,
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
