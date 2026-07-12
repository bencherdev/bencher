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
/// Stripe meter for monthly-active series (distinct testbed x benchmark x measure).
/// Backs the Pro tiered price (tier 1 flat fee plus per-series step-ups) and is billed
/// with `last` aggregation: after each report we post the org's cumulative
/// period-to-date series count, so the final post of the period is the period total and
/// a missed post self-heals on the next report.
pub const ACTIVE_SERIES_METER_NAME: &str = "active_series";
pub const RUNNER_MINUTES_METER_NAME: &str = "runner_minutes";

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
    /// cancel at the end of the current period. Only applies to metered
    /// subscriptions, not licensed (Self-Hosted) plans.
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
    /// When the metered subscription was created. The first (free-trial) billing
    /// period is the one that contains this timestamp.
    pub created: DateTime,
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
