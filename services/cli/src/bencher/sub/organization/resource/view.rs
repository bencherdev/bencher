use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonOrganization, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::CliOrganizationView,
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
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonOrganization = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .organization_get()
                        .organization(self.organization.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
