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
    pub checkout: NonEmpty,
    pub level: PlanLevel,
    pub entitlements: Option<Entitlements>,
    pub self_hosted: Option<OrganizationUuid>,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlanCreate) -> Result<Self, Self::Error> {
        let CliPlanCreate {
            org,
            checkout,
            level,
            entitlements,
            self_hosted,
            backend,
        } = create;
        Ok(Self {
            org,
            checkout: checkout.into(),
            level: level.into(),
            entitlements: entitlements.map(Into::into),
            self_hosted: self_hosted.map(Into::into),
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
            checkout,
            level,
            entitlements,
            self_hosted,
            ..
        } = create;
        #[allow(clippy::inconsistent_struct_constructor)]
        Self {
            checkout,
            level,
            entitlements,
            self_hosted,
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
