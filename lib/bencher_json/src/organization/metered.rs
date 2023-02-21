#![cfg(feature = "plus")]

use bencher_valid::{CardCvc, CardNumber, ExpirationMonth, ExpirationYear};
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_PRICE_NAME: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewPlan {
    pub card: JsonCard,
    pub level: JsonLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlan {
    pub organization: Uuid,
    pub user: Uuid,
    pub card: JsonCard,
    pub level: JsonLevel,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
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
pub enum JsonLevel {
    Team,
    Enterprise,
}

#[cfg(test)]
mod test {
    use bencher_valid::{ExpirationMonth, ExpirationYear};

    #[test]
    fn test_expiration_month_parse() {
        serde_json::from_str::<ExpirationMonth>("12").unwrap();
    }

    #[test]
    fn test_expiration_year_parse() {
        serde_json::from_str::<ExpirationYear>("2048").unwrap();
    }
}
