use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub organization: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanView> for View {
    type Error = CliError;

    fn try_from(view: CliPlanView) -> Result<Self, Self::Error> {
        let CliPlanView {
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
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_plan_get()
                    .organization(self.organization.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
