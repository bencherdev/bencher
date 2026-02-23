use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::CliOrganizationDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub organization: OrganizationResourceId,
    pub hard: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliOrganizationDelete) -> Result<Self, Self::Error> {
        let CliOrganizationDelete {
            organization,
            hard,
            backend,
        } = delete;
        Ok(Self {
            organization,
            hard,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut builder = client
                    .organization_delete()
                    .organization(self.organization.clone());
                if self.hard {
                    builder = builder.hard(self.hard);
                }
                builder.send().await
            })
            .await?;
        Ok(())
    }
}
