use stripe::{Customer, PaymentMethod};
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
}
