use bencher_client::types::JsonUpdatePlan;
use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub organization: OrganizationResourceId,
    pub cancel_at_period_end: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliPlanUpdate) -> Result<Self, Self::Error> {
        let CliPlanUpdate {
            organization,
            cancel,
            resume,
            backend,
        } = update;
        // `--cancel` schedules cancel-at-period-end; `--resume` clears it. The clap
        // ArgGroup requires exactly one, so no other combination can occur.
        let cancel_at_period_end = match (cancel, resume) {
            (true, false) => true,
            (false, true) => false,
            #[expect(
                clippy::unreachable,
                reason = "clap ArgGroup requires exactly one of `cancel`/`resume`"
            )]
            _ => unreachable!("Exactly one of `cancel` or `resume` is required"),
        };
        Ok(Self {
            organization,
            cancel_at_period_end,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdatePlan {
    fn from(update: Update) -> Self {
        let Update {
            cancel_at_period_end,
            ..
        } = update;
        Self {
            cancel_at_period_end,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_plan_patch()
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
