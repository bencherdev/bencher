use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::organization::CliOrganizationView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub organization: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationView> for View {
    type Error = CliError;

    fn try_from(view: CliOrganizationView) -> Result<Self, Self::Error> {
        let CliOrganizationView {
            organization,
            backend,
        } = view;
        Ok(Self {
            organization,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/organizations/{}", self.organization))
            .await?;
        Ok(())
    }
}
