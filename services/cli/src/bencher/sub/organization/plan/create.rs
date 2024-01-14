use std::convert::TryFrom;

use bencher_client::types::{Entitlements, JsonNewPlan, NonEmpty, OrganizationUuid, PlanLevel};
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::{CliPlanCreate, CliPlanLevel},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub customer: NonEmpty,
    pub payment_method: NonEmpty,
    pub level: PlanLevel,
    pub entitlements: Option<Entitlements>,
    pub organization: Option<OrganizationUuid>,
    pub i_agree: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlanCreate) -> Result<Self, Self::Error> {
        let CliPlanCreate {
            org,
            customer,
            payment_method,
            level,
            entitlements,
            organization,
            i_agree,
            backend,
        } = create;
        Ok(Self {
            org,
            customer: customer.into(),
            payment_method: payment_method.into(),
            level: level.into(),
            entitlements: entitlements.map(Into::into),
            organization: organization.map(Into::into),
            i_agree,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPlanLevel> for PlanLevel {
    fn from(level: CliPlanLevel) -> Self {
        match level {
            CliPlanLevel::Free => Self::Free,
            CliPlanLevel::Team => Self::Team,
            CliPlanLevel::Enterprise => Self::Enterprise,
        }
    }
}

impl From<Create> for JsonNewPlan {
    fn from(create: Create) -> Self {
        let Create {
            customer,
            payment_method,
            level,
            entitlements,
            organization,
            i_agree,
            ..
        } = create;
        #[allow(clippy::inconsistent_struct_constructor)]
        Self {
            customer,
            payment_method,
            level,
            entitlements,
            organization,
            i_agree,
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_plan_post()
                    .organization(self.org.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
