use stripe::{Customer, CustomerId, PaymentMethod, Subscription, SubscriptionItem};
use thiserror::Error;
use uuid::Uuid;

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
    #[error("No subscription for organization {0} and customer {1:#?}")]
    NoSubscription(Uuid, CustomerId),
    #[error("Multiple subscription items organization {0} and customer {1:#?}: {2:#?} {3:#?}")]
    MultipleSubscriptionItems(Uuid, CustomerId, SubscriptionItem, Vec<SubscriptionItem>),
    #[error("No subscription item for organization {0} and customer {1:#?}")]
    NoSubscriptionItem(Uuid, CustomerId),
}
