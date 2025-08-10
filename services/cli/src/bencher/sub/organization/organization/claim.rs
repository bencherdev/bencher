use bencher_client::types::JsonNewClaim;
use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::claim::CliOrganizationClaim,
};

#[derive(Debug, Clone)]
pub struct Claim {
    pub organization: OrganizationResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationClaim> for Claim {
    type Error = CliError;

    fn try_from(claim: CliOrganizationClaim) -> Result<Self, Self::Error> {
        let CliOrganizationClaim {
            organization,
            backend,
        } = claim;

        Ok(Self {
            organization,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Claim {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_claim_post()
                    .organization(self.organization.clone())
                    .body(JsonNewClaim { empty: () })
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
