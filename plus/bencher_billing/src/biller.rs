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

    pub async fn get_or_create_customer(
        &self,
        name: &NonEmpty,
        email: &Email,
    ) -> Result<Customer, BillingError> {
        if let Some(customer) = self.get_customer(email).await? {
            Ok(customer)
        } else {
            self.create_customer(name, email).await
        }
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

    // WARNING: Use caution when calling this directly as multiple users with the same email can be created
    // Use `get_or_create_customer` instead!
    async fn create_customer(
        &self,
        name: &NonEmpty,
        email: &Email,
    ) -> Result<Customer, BillingError> {
        let create_customer = CreateCustomer {
            name: Some(name.as_ref()),
            email: Some(email.as_ref()),
            ..Default::default()
        };
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
    use pretty_assertions::assert_eq;

    use crate::Biller;

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn test_billing_key() -> Option<String> {
        std::env::var(TEST_BILLING_KEY).ok()
    }

    #[tokio::test]
    async fn test_biller_create_customer() {
        let Some(billing_key) = test_billing_key() else {
            return;
        };

        let name = "Muriel Bagge".parse().unwrap();
        let email = format!("muriel.bagge.{}@nowhere.com", rand::random::<u64>())
            .parse()
            .unwrap();
        let biller = Biller::new(billing_key.parse().unwrap());
        assert!(biller.get_customer(&email).await.unwrap().is_none())
        let create_customer = biller.create_customer(&name, &email).await.unwrap();
        let get_customer = biller.get_customer(&email).await.unwrap().unwrap();
        assert_eq!(create_customer, get_customer);
        let get_or_create_customer = biller.get_or_create_customer(&name, &email).await.unwrap();
        assert_eq!(create_customer, get_or_create_customer);
    }
}
