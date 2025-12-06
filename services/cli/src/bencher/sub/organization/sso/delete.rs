use bencher_json::{OrganizationResourceId, SsoUuid};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::sso::CliSsoDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub organization: OrganizationResourceId,
    pub sso: SsoUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliSsoDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliSsoDelete) -> Result<Self, Self::Error> {
        let CliSsoDelete {
            organization,
            sso,
            backend,
        } = delete;
        Ok(Self {
            organization,
            sso,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_sso_delete()
                    .organization(self.organization.clone())
                    .sso(self.sso)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
