use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{BigInt, JsonCard, JsonNewPlan, OrganizationUuid, PlanLevel};
use bencher_json::{CardCvc, CardNumber, ExpirationMonth, ExpirationYear, JsonPlan, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::plan::{CliPlanCard, CliPlanCreate, CliPlanLevel},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub card: Card,
    pub level: PlanLevel,
    pub entitlements: Option<BigInt>,
    pub organization: Option<OrganizationUuid>,
    pub backend: Backend,
}

#[derive(Debug, Clone)]
pub struct Card {
    pub number: CardNumber,
    pub exp_month: ExpirationMonth,
    pub exp_year: ExpirationYear,
    pub cvc: CardCvc,
}

impl TryFrom<CliPlanCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlanCreate) -> Result<Self, Self::Error> {
        let CliPlanCreate {
            org,
            card,
            level,
            entitlements,
            organization,
            backend,
        } = create;
        Ok(Self {
            org,
            card: card.into(),
            level: level.into(),
            entitlements: entitlements.map(|e| (e * 1_000).into()),
            organization: organization.map(Into::into),
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

impl From<CliPlanCard> for Card {
    fn from(card: CliPlanCard) -> Self {
        let CliPlanCard {
            number,
            exp_month,
            exp_year,
            cvc,
        } = card;
        Self {
            number,
            exp_month,
            exp_year,
            cvc,
        }
    }
}

impl From<Create> for JsonNewPlan {
    fn from(create: Create) -> Self {
        let Create {
            card,
            level,
            entitlements,
            organization,
            ..
        } = create;
        Self {
            card: card.into(),
            level,
            entitlements,
            organization,
        }
    }
}

impl From<Card> for JsonCard {
    fn from(card: Card) -> Self {
        let Card {
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

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonPlan = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_plan_post()
                        .organization(self.org.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
