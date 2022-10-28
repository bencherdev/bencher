use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonNewOrganization;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::organization::CliOrganizationCreate,
    CliError,
};

const ORGANIZATIONS_PATH: &str = "/v0/organizations";

#[derive(Debug, Clone)]
pub struct Create {
    pub name: String,
    pub slug: Option<String>,
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
        let Create {
            name,
            slug,
            backend: _,
        } = create;
        Self { name, slug }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        let organization: JsonNewOrganization = self.clone().into();
        self.backend.post(ORGANIZATIONS_PATH, &organization).await?;
        Ok(())
    }
}
