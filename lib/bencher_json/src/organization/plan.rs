#![cfg(feature = "plus")]

use bencher_valid::{
    CardBrand, DateTime, Entitlements, ExpirationMonth, ExpirationYear, Jwt, LastFour, NonEmpty,
    PlanLevel, PlanStatus,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{system::payment::JsonCustomer, BigInt, OrganizationUuid};

pub const DEFAULT_PRICE_NAME: &str = "default";

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewPlan {
    pub checkout: NonEmpty,
    pub level: PlanLevel,
    pub entitlements: Option<Entitlements>,
    pub self_hosted: Option<OrganizationUuid>,
    pub remote: Option<bool>,
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
    pub level: PlanLevel,
    pub entitlements: Entitlements,
    pub issued_at: DateTime,
    pub expiration: DateTime,
    pub self_hosted: bool,
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
