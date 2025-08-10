use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanView,
};

#[derive(Debug)]
pub struct View {
    pub organization: OrganizationResourceId,
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
