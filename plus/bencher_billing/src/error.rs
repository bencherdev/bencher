use bencher_json::{PlanLevel, PlanStatus};
use stripe_billing::{Subscription, SubscriptionId};
use stripe_checkout::CheckoutSession;
use stripe_payment::PaymentMethodId;
use stripe_product::PriceId;
use stripe_shared::{Customer, CustomerId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to parse user UUID ({0}): {1}")]
    BadUserUuid(String, uuid::Error),
    #[error("Failed to parse organization UUID ({0}): {1}")]
    BadOrganizationUuid(String, uuid::Error),

    #[error("Failed to get checkout URL: {0:?}")]
    NoCheckoutUrl(Box<CheckoutSession>),
    #[error("Failed to to find checkout session subscription: {0:?}")]
    NoSubscription(Box<CheckoutSession>),

    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to send billing request: {0}")]
    Stripe(#[from] stripe::StripeError),
    #[error("Failed to build idempotency key: {0}")]
    IdempotencyKey(#[from] stripe::IdempotentKeyError),
    #[error("Email collision: {0:#?} {1:#?}")]
    EmailCollision(Box<Customer>, Vec<Customer>),
    #[error("Failed to find price: {0}")]
    PriceNotFound(String),
    #[error("Multiple subscriptions: {0:#?} {1:#?}")]
    MultipleSubscriptions(Box<Subscription>, Vec<Subscription>),
    #[error("No subscription item for {0}")]
    NoSubscriptionItem(SubscriptionId),
    #[error("No plan item for {0}: {1}")]
    PlanItem(SubscriptionId, PlanItemError),
    #[error("No organization for {0}")]
    NoOrganization(SubscriptionId),
    #[error("Failed to parse date/time for {0} {1}: {2}")]
    DateTime(SubscriptionId, i64, bencher_json::ValidError),
    #[error("No customer info for {0}")]
    NoCustomerInfo(CustomerId),
    #[error("No name for {0}")]
    NoName(CustomerId),
    #[error("No email for {0}")]
    NoEmail(CustomerId),
    #[error("No default payment method info for {0}")]
    NoDefaultPaymentMethodInfo(PaymentMethodId),
    #[error("No unit amount for {0}")]
    NoUnitAmount(PriceId),
}

/// Why a subscription was not attached to an organization.
#[derive(Debug, Error)]
pub enum AttachError {
    #[error("{0}")]
    Refused(#[from] AttachRefusal),
    #[error("Stripe has no subscription or checkout session {0}")]
    NotFound(String),
    #[error("{0}")]
    Billing(#[from] BillingError),
}

/// Why a subscription cannot be attached to an organization.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AttachRefusal {
    #[error("The subscription is {0}")]
    Terminal(PlanStatus),
    #[error("The subscription has {0}")]
    PlanItem(PlanItemError),
    #[error("The plan item's price ({0}) has no amount that the plan can show")]
    Price(String),
    #[error("The subscription has no item on the bare metal product")]
    NoBareMetal,
    #[error("A self-serve checkout sells only the Pro plan, not the {0} plan")]
    NotPro(PlanLevel),
    #[error("The subscription names no organization")]
    NoOrganization,
    #[error("The subscription names another organization ({0})")]
    OtherOrganization(String),
}

/// Why a subscription has no single plan item: one item on the Pro or Enterprise product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PlanItemError {
    #[error("no item on the Pro or Enterprise product")]
    Missing,
    #[error("more than one item on the Pro or Enterprise product")]
    Multiple,
}
