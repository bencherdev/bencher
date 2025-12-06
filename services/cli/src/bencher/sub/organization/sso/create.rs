use bencher_client::types::JsonNewSso;
use bencher_json::{NonEmpty, OrganizationResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::sso::CliSsoCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub organization: OrganizationResourceId,
    pub domain: NonEmpty,
    pub backend: AuthBackend,
}

impl TryFrom<CliSsoCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliSsoCreate) -> Result<Self, Self::Error> {
        let CliSsoCreate {
            organization,
            domain,
            backend,
        } = create;
        Ok(Self {
            organization,
            domain,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewSso {
    fn from(create: Create) -> Self {
        let Create { domain, .. } = create;
        Self {
            domain: domain.into(),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_sso_post()
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
