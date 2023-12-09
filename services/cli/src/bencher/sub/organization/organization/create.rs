use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewOrganization;
use bencher_json::{NonEmpty, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::CliOrganizationCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub backend: Backend,
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

#[async_trait]
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
