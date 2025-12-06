use bencher_json::{OrganizationResourceId, SsoUuid};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::organization::sso::CliSsoView,
};

#[derive(Debug)]
pub struct View {
    pub organization: OrganizationResourceId,
    pub sso: SsoUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliSsoView> for View {
    type Error = CliError;

    fn try_from(view: CliSsoView) -> Result<Self, Self::Error> {
        let CliSsoView {
            organization,
            sso,
            backend,
        } = view;
        Ok(Self {
            organization,
            sso,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_sso_get()
                    .organization(self.organization.clone())
                    .sso(self.sso)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
