use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateOrganization;
use bencher_json::{JsonOrganization, NonEmpty, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::CliOrganizationUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub organization: ResourceId,
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliOrganizationUpdate) -> Result<Self, Self::Error> {
        let CliOrganizationUpdate {
            organization,
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            organization,
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateOrganization {
    fn from(update: Update) -> Self {
        let Update { name, slug, .. } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonOrganization = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .organization_patch()
                        .organization(self.organization.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
