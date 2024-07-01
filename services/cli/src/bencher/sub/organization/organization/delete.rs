use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::CliOrganizationDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub organization: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliOrganizationDelete) -> Result<Self, Self::Error> {
        let CliOrganizationDelete {
            organization,
            backend,
        } = delete;
        Ok(Self {
            organization,
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
                    .organization_delete()
                    .organization(self.organization.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
