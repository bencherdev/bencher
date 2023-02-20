use stripe::{Customer, PaymentMethod, Subscription, SubscriptionId, SubscriptionItem};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("Failed to send billing request: {0}")]
    Stripe(#[from] stripe::StripeError),
    #[error("Failed to parse ID: {0}")]
    StripeId(#[from] stripe::ParseIdError),
    #[error("Email collision: {0:#?} {1:#?}")]
    EmailCollision(Customer, Vec<Customer>),
    #[error("Multiple payment methods: {0:#?} {1:#?}")]
    MultiplePaymentMethods(PaymentMethod, Vec<PaymentMethod>),
    #[error("Failed to find price: {0}")]
    PriceNotFound(String),
    #[error("Subscription quantity set to zero: {0}")]
    QuantityZero(u64),
    #[error("Multiple subscriptions: {0:#?} {1:#?}")]
    MultipleSubscriptions(Subscription, Vec<Subscription>),
    #[error("Multiple subscription items for {0}: {1:#?} {2:#?}")]
    MultipleSubscriptionItems(SubscriptionId, SubscriptionItem, Vec<SubscriptionItem>),
    #[error("No subscription item for {0}")]
    NoSubscriptionItem(SubscriptionId),
}
