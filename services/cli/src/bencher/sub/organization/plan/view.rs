use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonPlan, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::plan::CliPlanView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub organization: ResourceId,
    pub backend: Backend,
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
        let _json: JsonPlan = self
            .backend
            .send_with(|client| async move {
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
