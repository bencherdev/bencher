use stripe::{
    Customer, CustomerId, PaymentMethod, PaymentMethodId, PriceId, ProductId, Subscription,
    SubscriptionItem, SubscriptionItemId,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to parse metered plan ID: {0}")]
    MeteredPlanId(stripe::ParseIdError),
    #[error("Failed to parse licensed plan ID: {0}")]
    LicensedPlanId(stripe::ParseIdError),
    #[error("Failed to parse user UUID ({0}): {1}")]
    BadUserUuid(String, uuid::Error),
    #[error("Failed to parse organization UUID ({0}): {1}")]
    BadOrganizationUuid(String, uuid::Error),

    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
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
    #[error("Cannot create a subscription for the free tier")]
    ProductLevelFree,
    #[error("Multiple subscriptions: {0:#?} {1:#?}")]
    MultipleSubscriptions(Subscription, Vec<Subscription>),
    #[error("Multiple subscription items for {0}: {1:#?} {2:#?}")]
    MultipleSubscriptionItems(
        crate::biller::PlanId,
        SubscriptionItem,
        Vec<SubscriptionItem>,
    ),
    #[error("No subscription item for {0}")]
    NoSubscriptionItem(crate::biller::PlanId),
    #[error("No organization for {0}")]
    NoOrganization(crate::biller::PlanId),
    #[error("Failed to parse date/time for {0} {1}: {2}")]
    DateTime(crate::biller::PlanId, i64, bencher_json::ValidError),
    #[error("No customer info for {0}")]
    NoCustomerInfo(CustomerId),
    #[error("No UUID for {0}")]
    NoUuid(CustomerId),
    #[error("No name for {0}")]
    NoName(CustomerId),
    #[error("No email for {0}")]
    NoEmail(CustomerId),
    #[error("No default payment method for {0}")]
    NoDefaultPaymentMethod(crate::biller::PlanId),
    #[error("No default payment method info for {0}")]
    NoDefaultPaymentMethodInfo(PaymentMethodId),
    #[error("No card details for {0}")]
    NoCardDetails(PaymentMethodId),
    #[error("No price for {0}")]
    NoPrice(SubscriptionItemId),
    #[error("No unit amount for {0}")]
    NoUnitAmount(PriceId),
    #[error("No product for {0}")]
    NoProduct(PriceId),
    #[error("No product info for {0}")]
    NoProductInfo(ProductId),
    #[error("No product name for {0}")]
    NoProductName(ProductId),
}
