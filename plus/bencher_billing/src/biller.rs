use bencher_valid::Secret;
use stripe::{Client, CreateCustomer, Customer};

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
        create_customer: CreateCustomer,
    ) -> Result<Customer, BillingError> {
        Customer::create(&self, create_customer)
            .await
            .map_err(Into::into)
    }
}
