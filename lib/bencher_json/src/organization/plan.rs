#![cfg(feature = "plus")]

use bencher_valid::{
    CardBrand, CardCvc, CardNumber, DateTime, Email, ExpirationMonth, ExpirationYear, Jwt,
    LastFour, PlanLevel, PlanStatus, UserName,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BigInt, OrganizationUuid, UserUuid};

pub const DEFAULT_PRICE_NAME: &str = "default";

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewPlan {
    pub card: JsonCard,
    pub level: PlanLevel,
    pub entitlements: Option<BigInt>,
    pub organization: Option<OrganizationUuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCard {
    pub number: CardNumber,
    pub exp_month: ExpirationMonth,
    pub exp_year: ExpirationYear,
    pub cvc: CardCvc,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlan {
    pub organization: OrganizationUuid,
    pub customer: JsonCustomer,
    pub card: JsonCardDetails,
    pub level: PlanLevel,
    pub unit_amount: BigInt,
    pub current_period_start: DateTime,
    pub current_period_end: DateTime,
    pub status: PlanStatus,
    pub license: Option<JsonLicense>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCustomer {
    pub uuid: UserUuid,
    pub name: UserName,
    pub email: Email,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCardDetails {
    pub brand: CardBrand,
    pub last_four: LastFour,
    pub exp_month: ExpirationMonth,
    pub exp_year: ExpirationYear,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLicense {
    pub key: Jwt,
    pub organization: OrganizationUuid,
    pub entitlements: BigInt,
    pub issued_at: DateTime,
    pub expiration: DateTime,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
