use bencher_valid::{Email, NonEmpty, Secret};
use stripe::{
    AttachPaymentMethod, Client, CreateCustomer, CreatePaymentMethod, Customer, ListCustomers,
    PaymentMethod,
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

    pub async fn new_customer(
        &self,
        name: &NonEmpty,
        email: &Email,
    ) -> Result<Customer, BillingError> {
        if self.get_customer(email).await?.is_some() {
            return Err(BillingError::EmailExists(email.clone()));
        }

        let create_customer = CreateCustomer {
            name: Some(name.as_ref()),
            email: Some(email.as_ref()),
            ..Default::default()
        };
        Customer::create(&self.client, create_customer)
            .await
            .map_err(Into::into)
    }

    pub async fn get_customer(&self, email: &Email) -> Result<Option<Customer>, BillingError> {
        let list_customers = ListCustomers {
            email: Some(email.as_ref()),
            ..Default::default()
        };
        let mut customers = Customer::list(&self.client, &list_customers).await?;

        if let Some(customer) = customers.data.pop() {
            if customers.data.is_empty() {
                Ok(Some(customer))
            } else {
                Err(BillingError::EmailCollision(customer, customers.data))
            }
        } else {
            Ok(None)
        }
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
            .new_customer(
                &"Muriel Bagge".parse().unwrap(),
                &"muriel.bagge@nowhere.com".parse().unwrap(),
            )
            .await
            .unwrap();
    }
}
