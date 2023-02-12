use stripe::{Customer, PaymentMethod};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("Failed to send billing request: {0}")]
    Stripe(#[from] stripe::StripeError),
    #[error("Email collision: {0:#?} {1:#?}")]
    EmailCollision(Customer, Vec<Customer>),
    #[error("Multiple payment methods: {0:#?} {1:#?}")]
    MultiplePaymentMethods(PaymentMethod, Vec<PaymentMethod>),
}
