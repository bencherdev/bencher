use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonCard, JsonNewPlan};
use bencher_json::{CardCvc, CardNumber, ExpirationMonth, ExpirationYear, JsonPlan, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::plan::{CliPlanCard, CliPlanCreate},
    CliError,
};

use super::level::Level;

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
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
            org,
            card,
            level,
            backend,
        } = create;
        Ok(Self {
            org,
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
