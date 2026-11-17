#![cfg(feature = "plus")]

use bencher_valid::{
    CardBrand, DateTime, Entitlements, ExpirationMonth, ExpirationYear, Jwt, LastFour, NonEmpty,
    PlanLevel, PlanStatus,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{BigInt, OrganizationUuid, system::payment::JsonCustomer};

pub const DEFAULT_PRICE_NAME: &str = "default";
/// Stripe meter for monthly-active series (distinct testbed x benchmark x measure).
///
/// Backs every metered plan's price and is billed with `last` aggregation: after each
/// report we post the org's cumulative period-to-date series count, so the final post of
/// the period is the period total and a missed post self-heals on the next report.
pub const ACTIVE_SERIES_METER_NAME: &str = "active_series";
pub const RUNNER_MINUTES_METER_NAME: &str = "runner_minutes";

/// A plan to attach to an organization: the return from a self-serve checkout, or a
/// subscription that a server admin attaches.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonNewPlan {
    Checkout(JsonPlanCheckout),
    Subscription(JsonPlanSubscription),
}

/// The return from a self-serve checkout.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlanCheckout {
    /// The Stripe checkout session that created the subscription.
    pub checkout: NonEmpty,
}

/// A subscription that a server admin attaches.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlanSubscription {
    /// The Stripe subscription.
    pub subscription: NonEmpty,
}

/// Hand written so a body that gives both `checkout` and `subscription`, or neither, is
/// refused rather than read as whichever variant matches first. Any other field is
/// ignored.
impl<'de> Deserialize<'de> for JsonNewPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NewPlan {
            checkout: Option<NonEmpty>,
            subscription: Option<NonEmpty>,
        }

        let NewPlan {
            checkout,
            subscription,
        } = NewPlan::deserialize(deserializer)?;
        match (checkout, subscription) {
            (Some(checkout), None) => Ok(Self::Checkout(JsonPlanCheckout { checkout })),
            (None, Some(subscription)) => {
                Ok(Self::Subscription(JsonPlanSubscription { subscription }))
            },
            (Some(_), Some(_)) => Err(de::Error::custom(
                "expected one of `checkout` or `subscription`, not both",
            )),
            (None, None) => Err(de::Error::custom(
                "expected one of `checkout` or `subscription`",
            )),
        }
    }
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
    /// The Bencher user who pays for the plan. `None` when the Stripe customer was made
    /// in Stripe, not by Bencher.
    pub customer: Option<JsonCustomer>,
    /// The card that pays for the plan. `None` when the plan is billed by invoice, or by a
    /// payment method that is not a card.
    pub card: Option<JsonCardDetails>,
    /// How Stripe collects each payment for the plan.
    pub collection_method: CollectionMethod,
    pub level: PlanLevel,
    pub unit_amount: BigInt,
    /// The tiered price ladder of the plan's own price, ordered ascending by `up_to`.
    /// `None` for a flat price.
    pub tiers: Option<Vec<JsonPriceTier>>,
    /// How the tiers price the active series. `None` for a flat price.
    pub tiers_mode: Option<TiersMode>,
    /// The bare metal price per runner minute, in cents, from the subscription's own bare
    /// metal item. `None` when the subscription has no bare metal item, or its price has
    /// no per minute amount.
    pub runner_minute: Option<f64>,
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

/// How Stripe collects each payment for a plan.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CollectionMethod {
    /// Stripe charges the plan's payment method.
    ChargeAutomatically,
    /// Stripe sends an invoice, and the customer pays it.
    SendInvoice,
}

/// How a tiered price charges for active series.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TiersMode {
    /// Each tier charges for the series inside it, and the charges add up.
    Graduated,
    /// The tier that holds the whole count charges for every series.
    Volume,
}

/// A Stripe billing portal session where the plan's customer manages their payment method.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlanPortal {
    pub url: String,
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

/// One tier of a Stripe tiered price, surfaced so the Console can render the price
/// ladder from the billed source of truth instead of hardcoding it.
///
/// Field order mirrors Stripe's tier shape: `up_to`, then `unit_amount`, then
/// `flat_amount`. A tier may carry both a `unit_amount` and a `flat_amount` (Stripe
/// allows it); the two are independent and additive, not mutually exclusive. Today the
/// Pro price sets only `flat_amount`.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPriceTier {
    /// Inclusive upper bound (active-series count) for this tier. `None` is the
    /// unbounded top tier.
    pub up_to: Option<u32>,
    /// Per-series price (cents) charged within this tier. May coexist with `flat_amount`.
    pub unit_amount: Option<BigInt>,
    /// Flat price (cents) for the whole tier. May coexist with `unit_amount`.
    pub flat_amount: Option<BigInt>,
}

#[cfg(test)]
mod tests {
    use bencher_valid::{ExpirationMonth, ExpirationYear};

    use super::{JsonNewPlan, JsonPlanCheckout, JsonPlanSubscription};

    #[test]
    fn expiration_month_parse() {
        serde_json::from_str::<ExpirationMonth>("12").unwrap();
    }

    #[test]
    fn expiration_year_parse() {
        serde_json::from_str::<ExpirationYear>("2048").unwrap();
    }

    // Fails if a checkout return reads as an attach, an attach reads as a checkout
    // return, or a body that gives both or neither is taken.
    #[test]
    fn new_plan_takes_one_of_checkout_or_subscription() {
        let plan: JsonNewPlan = serde_json::from_str(r#"{"checkout":"cs_test"}"#).unwrap();
        let JsonNewPlan::Checkout(JsonPlanCheckout { checkout }) = plan else {
            panic!("not a checkout: {plan:?}");
        };
        assert_eq!(checkout.as_ref(), "cs_test");

        let plan: JsonNewPlan = serde_json::from_str(r#"{"subscription":"sub_test"}"#).unwrap();
        let JsonNewPlan::Subscription(JsonPlanSubscription { subscription }) = plan else {
            panic!("not a subscription: {plan:?}");
        };
        assert_eq!(subscription.as_ref(), "sub_test");

        serde_json::from_str::<JsonNewPlan>(r#"{"checkout":"cs_test","subscription":"sub_test"}"#)
            .unwrap_err();
        serde_json::from_str::<JsonNewPlan>("{}").unwrap_err();
        serde_json::from_str::<JsonNewPlan>(r#"{"checkout":""}"#).unwrap_err();
    }

    // Fails if a return from an earlier console, which still names its level, is refused.
    #[test]
    fn new_plan_ignores_retired_fields() {
        let plan: JsonNewPlan =
            serde_json::from_str(r#"{"checkout":"cs_test","level":"pro","remote":false}"#).unwrap();
        assert!(matches!(plan, JsonNewPlan::Checkout(_)), "{plan:?}");
    }
}
