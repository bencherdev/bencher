#![cfg(feature = "plus")]

use bencher_valid::{
    CardCvc, CardNumber, Email, ExpirationMonth, ExpirationYear, NonEmpty, UserName,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetered {
    pub card: JsonCard,
    pub plan: JsonPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCard {
    pub number: CardNumber,
    pub exp_month: ExpirationMonth,
    pub exp_year: ExpirationYear,
    pub cvc: CardCvc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename = "snake_case")]
pub enum JsonPlan {
    Team(NonEmpty),
    Enterprise(NonEmpty),
}
