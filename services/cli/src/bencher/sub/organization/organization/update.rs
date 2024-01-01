use std::convert::TryFrom;

use async_trait::async_trait;
#[cfg(feature = "plus")]
use bencher_client::types::JsonOrganizationPatchNull;
use bencher_client::types::{JsonOrganizationPatch, JsonUpdateOrganization};
use bencher_json::{ResourceId, ResourceName, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::CliOrganizationUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub organization: ResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
    #[cfg(feature = "plus")]
    #[cfg_attr(feature = "plus", allow(clippy::option_option))]
    pub license: Option<Option<bencher_json::Jwt>>,
    pub backend: AuthBackend,
}

impl TryFrom<CliOrganizationUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliOrganizationUpdate) -> Result<Self, Self::Error> {
        let CliOrganizationUpdate {
            organization,
            name,
            slug,
            #[cfg(feature = "plus")]
            license,
            backend,
        } = create;
        Ok(Self {
            organization,
            name,
            slug,
            #[cfg(feature = "plus")]
            license,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateOrganization {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            #[cfg(feature = "plus")]
            license,
            ..
        } = update;
        #[cfg(not(feature = "plus"))]
        return Self {
            subtype_0: Some(JsonOrganizationPatch {
                name: name.map(Into::into),
                slug: slug.map(Into::into),
                license: None,
            }),
            subtype_1: None,
        };
        #[cfg(feature = "plus")]
        return match license {
            Some(Some(license)) => Self {
                subtype_0: Some(JsonOrganizationPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    license: Some(license.into()),
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(JsonOrganizationPatchNull {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    license: (),
                }),
            },
            None => Self {
                subtype_0: Some(JsonOrganizationPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    license: None,
                }),
                subtype_1: None,
            },
        };
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .organization_patch()
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
