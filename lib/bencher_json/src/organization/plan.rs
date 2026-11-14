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
    /// The Self-Hosted organization that the subscription buys a license for. Bencher Cloud
    /// signs the license, and the organization that the subscription is attached to keeps it
    /// to copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_hosted: Option<OrganizationUuid>,
}

/// Hand written so a body that gives both `checkout` and `subscription`, or neither, is
/// refused rather than read as whichever variant matches first, and so is a checkout that
/// names a Self-Hosted organization. Any other field is ignored.
impl<'de> Deserialize<'de> for JsonNewPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NewPlan {
            checkout: Option<NonEmpty>,
            subscription: Option<NonEmpty>,
            self_hosted: Option<OrganizationUuid>,
        }

        let NewPlan {
            checkout,
            subscription,
            self_hosted,
        } = NewPlan::deserialize(deserializer)?;
        match (checkout, subscription, self_hosted) {
            (Some(checkout), None, None) => Ok(Self::Checkout(JsonPlanCheckout { checkout })),
            (Some(_), None, Some(_)) => Err(de::Error::custom(
                "expected `self_hosted` only with `subscription`, since a checkout sells Bencher Cloud's Pro plan",
            )),
            (None, Some(subscription), self_hosted) => {
                Ok(Self::Subscription(JsonPlanSubscription {
                    subscription,
                    self_hosted,
                }))
            },
            (Some(_), Some(_), _) => Err(de::Error::custom(
                "expected one of `checkout` or `subscription`, not both",
            )),
            (None, None, _) => Err(de::Error::custom(
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

/// A Bencher Plus Enterprise license for a Self-Hosted organization.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLicense {
    /// The license key.
    pub key: Jwt,
    /// The Self-Hosted organization the license is for.
    pub organization: OrganizationUuid,
    /// The Benchmark Series the license covers.
    pub series: Entitlements,
    /// When the license was issued.
    pub issued_at: DateTime,
    /// The end of the license's paid term.
    pub term_end: DateTime,
    /// When the license expires, at the end of its grace after the term ends.
    pub expiration: DateTime,
}

/// What the organization of a new report needs to know about its license, on the response
/// that created the report. Absent when there is nothing to know.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReportLicense {
    /// Present when the organization's active Benchmark Series are above the license's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<JsonLicenseOver>,
    /// Present when the license is past the end of its paid term, in its grace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace: Option<JsonLicenseGrace>,
}

/// An organization with more active Benchmark Series than its license covers.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLicenseOver {
    /// The organization's Benchmark Series active in the last 30 days.
    pub active_series: u32,
    /// The Benchmark Series the license covers.
    pub series: Entitlements,
}

/// A license past the end of its paid term, in its grace.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLicenseGrace {
    /// The end of the license's paid term.
    pub term_end: DateTime,
    /// When the license expires.
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
    use crate::OrganizationUuid;

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
        let JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription,
            self_hosted,
        }) = plan
        else {
            panic!("not a subscription: {plan:?}");
        };
        assert_eq!(subscription.as_ref(), "sub_test");
        assert_eq!(self_hosted, None);

        serde_json::from_str::<JsonNewPlan>(r#"{"checkout":"cs_test","subscription":"sub_test"}"#)
            .unwrap_err();
        serde_json::from_str::<JsonNewPlan>("{}").unwrap_err();
        serde_json::from_str::<JsonNewPlan>(r#"{"checkout":""}"#).unwrap_err();
    }

    // Fails if an attach for a Self-Hosted organization loses the organization, takes one that
    // is not a UUID, or if a checkout return that names one is taken, since a self-serve
    // checkout sells Pro on Bencher Cloud only.
    #[test]
    fn new_plan_takes_self_hosted_with_a_subscription_only() {
        let self_hosted = OrganizationUuid::new();
        let plan: JsonNewPlan = serde_json::from_value(serde_json::json!({
            "subscription": "sub_test",
            "self_hosted": self_hosted,
        }))
        .unwrap();
        let JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription,
            self_hosted: named,
        }) = plan
        else {
            panic!("not a subscription: {plan:?}");
        };
        assert_eq!(subscription.as_ref(), "sub_test");
        assert_eq!(named, Some(self_hosted));

        serde_json::from_value::<JsonNewPlan>(serde_json::json!({
            "subscription": "sub_test",
            "self_hosted": "not-a-uuid",
        }))
        .unwrap_err();
        let error = serde_json::from_value::<JsonNewPlan>(serde_json::json!({
            "checkout": "cs_test",
            "self_hosted": self_hosted,
        }))
        .unwrap_err();
        assert!(error.to_string().contains("self_hosted"), "{error}");
    }

    // Fails if an attach for a Self-Hosted organization is sent without it, or one for Bencher
    // Cloud is sent naming one.
    #[test]
    fn new_plan_sends_self_hosted_only_when_given() {
        let self_hosted = OrganizationUuid::new();
        let plan = |self_hosted| {
            serde_json::to_value(JsonNewPlan::Subscription(JsonPlanSubscription {
                subscription: "sub_test".parse().unwrap(),
                self_hosted,
            }))
            .unwrap()
        };
        assert_eq!(
            plan(Some(self_hosted)),
            serde_json::json!({ "subscription": "sub_test", "self_hosted": self_hosted })
        );
        assert_eq!(
            plan(None),
            serde_json::json!({ "subscription": "sub_test" })
        );
    }

    // Fails if a return from an earlier console, which still names its level, is refused.
    #[test]
    fn new_plan_ignores_retired_fields() {
        let plan: JsonNewPlan =
            serde_json::from_str(r#"{"checkout":"cs_test","level":"pro","remote":false}"#).unwrap();
        assert!(matches!(plan, JsonNewPlan::Checkout(_)), "{plan:?}");
    }
}
