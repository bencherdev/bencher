use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    organization::metered::{JsonCard, JsonNewPlan},
    CardCvc, CardNumber, ExpirationMonth, ExpirationYear, ResourceId,
};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::plan::{CliPlanCard, CliPlanCreate},
    CliError,
};

use super::level::Level;

#[derive(Debug, Clone)]
pub struct Create {
    pub organization: ResourceId,
    pub card: Card,
    pub level: Level,
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
            organization,
            card,
            level,
            backend,
        } = create;
        Ok(Self {
            organization,
            card: card.into(),
            level: level.into(),
            backend: backend.try_into()?,
        })
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
        let Create { card, level, .. } = create;
        Self {
            card: card.into(),
            level: level.into(),
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
            number,
            exp_month,
            exp_year,
            cvc,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let plan: JsonNewPlan = self.clone().into();
        self.backend
            .post(
                &format!("/v0/organizations/{}/plan", self.organization),
                &plan,
            )
            .await?;
        Ok(())
    }
}
