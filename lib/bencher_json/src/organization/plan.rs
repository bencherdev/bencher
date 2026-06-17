#![cfg(feature = "plus")]

use bencher_valid::{
    CardBrand, DateTime, Entitlements, ExpirationMonth, ExpirationYear, Jwt, LastFour, NonEmpty,
    PlanLevel, PlanStatus,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BigInt, OrganizationUuid, system::payment::JsonCustomer};

pub const DEFAULT_PRICE_NAME: &str = "default";
pub const METRICS_METER_NAME: &str = "metrics";
pub const RUNNER_MINUTES_METER_NAME: &str = "runner_minutes";

/// The fungible usage credit included with Pro each billing period, in cents (`$20.00`).
/// Drawn down across metrics and bare metal; expires at period end.
pub const PRO_INCLUDED_CREDIT_CENTS: i64 = 2_000;

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
pub struct JsonUpdatePlan {
    /// Update the subscription's scheduled cancellation. Set to `false` to resume
    /// a plan scheduled to cancel at period end, or `true` to schedule it to
    /// cancel at the end of the current period. Only applies to metered (Pro)
    /// plans.
    pub cancel_at_period_end: bool,
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
    /// Whether the subscription is set to cancel at the end of the current period
    /// (the org keeps access until `current_period_end`, then downgrades to Free).
    pub cancel_at_period_end: bool,
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
}

#[cfg(test)]
mod tests {
    use bencher_valid::{ExpirationMonth, ExpirationYear};

    #[test]
    fn expiration_month_parse() {
        serde_json::from_str::<ExpirationMonth>("12").unwrap();
    }

    #[test]
    fn expiration_year_parse() {
        serde_json::from_str::<ExpirationYear>("2048").unwrap();
    }
}
