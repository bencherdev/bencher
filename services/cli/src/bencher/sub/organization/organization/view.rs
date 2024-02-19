use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::CliOrganizationView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub organization: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationView> for View {
    type Error = CliError;

    fn try_from(view: CliOrganizationView) -> Result<Self, Self::Error> {
        let CliOrganizationView {
            organization,
            backend,
        } = view;
        Ok(Self {
            organization,
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
                    .organization_get()
                    .organization(self.organization.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
