use bencher_valid::Email;
use stripe::Customer;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("Failed to send billing request: {0}")]
    Stripe(#[from] stripe::StripeError),
    #[error("Email already exists: {0}")]
    EmailExists(Email),
    #[error("Email collision: {0:#?} {1:#?}")]
    EmailCollision(Customer, Vec<Customer>),
}
