use std::convert::TryFrom;

use bencher_client::types::JsonNewOrganization;
use bencher_json::{ResourceName, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::CliOrganizationCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub name: ResourceName,
    pub slug: Option<Slug>,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliOrganizationCreate) -> Result<Self, Self::Error> {
        let CliOrganizationCreate {
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewOrganization {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(
                |client| async move { client.organization_post().body(self.clone()).send().await },
            )
            .await?;
        Ok(())
    }
}
