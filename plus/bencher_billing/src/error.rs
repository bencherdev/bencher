use stripe_billing::{Subscription, SubscriptionId, SubscriptionItem};
use stripe_checkout::CheckoutSession;
use stripe_payment::PaymentMethodId;
use stripe_product::PriceId;
use stripe_shared::{Customer, CustomerId, ProductId};
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
    #[error("Email collision: {0:#?} {1:#?}")]
    EmailCollision(Box<Customer>, Vec<Customer>),
    #[error("Failed to find price: {0}")]
    PriceNotFound(String),
    #[error("Cannot create a subscription for the free tier")]
    ProductLevelFree,
    #[error("Multiple subscriptions: {0:#?} {1:#?}")]
    MultipleSubscriptions(Box<Subscription>, Vec<Subscription>),
    #[error("Multiple subscription items for {0}: {1:#?} {2:#?}")]
    MultipleSubscriptionItems(SubscriptionId, Box<SubscriptionItem>, Vec<SubscriptionItem>),
    #[error("No subscription item for {0}")]
    NoSubscriptionItem(SubscriptionId),
    #[error("No organization for {0}")]
    NoOrganization(SubscriptionId),
    #[error("Failed to parse date/time for {0} {1}: {2}")]
    DateTime(SubscriptionId, i64, bencher_json::ValidError),
    #[error("No customer info for {0}")]
    NoCustomerInfo(CustomerId),
    #[error("No UUID for {0}")]
    NoUuid(CustomerId),
    #[error("No name for {0}")]
    NoName(CustomerId),
    #[error("No email for {0}")]
    NoEmail(CustomerId),
    #[error("No default payment method for {0}")]
    NoDefaultPaymentMethod(SubscriptionId),
    #[error("No default payment method info for {0}")]
    NoDefaultPaymentMethodInfo(PaymentMethodId),
    #[error("No card details for {0}")]
    NoCardDetails(PaymentMethodId),
    #[error("No unit amount for {0}")]
    NoUnitAmount(PriceId),
    #[error("No product info for {0}")]
    NoProductInfo(ProductId),
}
