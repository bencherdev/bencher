use bencher_valid::Secret;
use stripe::{
    AttachPaymentMethod, Client, CreateCustomer, CreatePaymentMethod, Customer, PaymentMethod,
};

use crate::BillingError;

pub struct Biller {
    client: Client,
}

impl Biller {
    pub fn new(secret_key: Secret) -> Self {
        let client = Client::new(secret_key);
        Self { client }
    }

    pub async fn new_customer<'c>(
        &self,
        create_customer: CreateCustomer<'c>,
    ) -> Result<Customer, BillingError> {
        Customer::create(&self.client, create_customer)
            .await
            .map_err(Into::into)
    }

    pub async fn new_payment_method<'c>(
        &self,
        create_payment_method: CreatePaymentMethod<'c>,
    ) -> Result<PaymentMethod, BillingError> {
        PaymentMethod::create(&self.client, create_payment_method)
            .await
            .map_err(Into::into)
    }

    pub async fn customer_payment_method(
        &self,
        customer: &Customer,
        payment_method: &PaymentMethod,
    ) -> Result<PaymentMethod, BillingError> {
        PaymentMethod::attach(
            &self.client,
            &payment_method.id,
            AttachPaymentMethod {
                customer: customer.id.clone(),
            },
        )
        .await
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use stripe::CreateCustomer;

    use crate::Biller;

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn test_billing_key() -> String {
        std::env::var(TEST_BILLING_KEY).unwrap()
    }

    #[tokio::test]
    async fn test_biller_create_customer() {
        let billing_key = test_billing_key();
        let biller = Biller::new(billing_key.parse().unwrap());
        let _customer = biller
            .new_customer(CreateCustomer {
                name: Some("Muriel Bagge"),
                email: Some("muriel.bagge@nowhere.com"),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}
