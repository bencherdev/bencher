use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonNewOrganization, NonEmpty, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::CliOrganizationCreate,
    CliError,
};

const ORGANIZATIONS_PATH: &str = "/v0/organizations";

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
            name: NonEmpty::from_str(&name)?,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewOrganization {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self { name, slug }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let organization: JsonNewOrganization = self.clone().into();
        self.backend.post(ORGANIZATIONS_PATH, &organization).await?;
        Ok(())
    }
}
