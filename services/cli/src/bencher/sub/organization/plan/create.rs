use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{
    Entitlements, JsonCard, JsonCustomer, JsonNewPlan, OrganizationUuid, PlanLevel,
};
use bencher_json::{JsonPlan, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::plan::{CliPlanCard, CliPlanCreate, CliPlanCustomer, CliPlanLevel},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub customer: JsonCustomer,
    pub card: JsonCard,
    pub level: PlanLevel,
    pub entitlements: Option<Entitlements>,
    pub organization: Option<OrganizationUuid>,
    pub i_agree: bool,
    pub backend: Backend,
}

impl TryFrom<CliPlanCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlanCreate) -> Result<Self, Self::Error> {
        let CliPlanCreate {
            org,
            customer,
            card,
            level,
            entitlements,
            organization,
            i_agree,
            backend,
        } = create;
        Ok(Self {
            org,
            customer: customer.into(),
            card: card.into(),
            level: level.into(),
            entitlements: entitlements.map(Into::into),
            organization: organization.map(Into::into),
            i_agree,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPlanCustomer> for JsonCustomer {
    fn from(customer: CliPlanCustomer) -> Self {
        let CliPlanCustomer { uuid, name, email } = customer;
        Self {
            uuid: uuid.into(),
            name: name.into(),
            email: email.into(),
        }
    }
}

impl From<CliPlanCard> for JsonCard {
    fn from(card: CliPlanCard) -> Self {
        let CliPlanCard {
            number,
            exp_month,
            exp_year,
            cvc,
        } = card;
        Self {
            number: number.into(),
            exp_month: exp_month.into(),
            exp_year: exp_year.into(),
            cvc: cvc.into(),
        }
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
            card,
            level,
            entitlements,
            organization,
            i_agree,
            ..
        } = create;
        #[allow(clippy::inconsistent_struct_constructor)]
        Self {
            customer,
            card,
            level,
            entitlements,
            organization,
            i_agree,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonPlan = self
            .backend
            .send_with(|client| async move {
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
