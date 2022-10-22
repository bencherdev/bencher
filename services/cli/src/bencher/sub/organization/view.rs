use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::CliOrganizationView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub org: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationView> for View {
    type Error = CliError;

    fn try_from(view: CliOrganizationView) -> Result<Self, Self::Error> {
        let CliOrganizationView { org, backend } = view;
        Ok(Self {
            org,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/organizations/{}", self.org))
            .await?;
        Ok(())
    }
}
