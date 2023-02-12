use bencher_valid::{Email, NonEmpty, Secret};
use chrono::Datelike;
use stripe::{
    AttachPaymentMethod, Client, CreateCustomer, CreatePaymentMethod, CreatePaymentMethodCardUnion,
    ListCustomers, ListPaymentMethods, PaymentMethod, PaymentMethodTypeFilter,
};
pub use stripe::{CardDetailsParams as PaymentCard, Customer};

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

    pub async fn get_or_create_payment_method(
        &self,
        customer: &Customer,
        payment_card: PaymentCard,
    ) -> Result<PaymentMethod, BillingError> {
        if let Some(payment_method) = self.get_payment_method(customer).await? {
            Ok(payment_method)
        } else {
            let payment_method = self.create_payment_method(payment_card).await?;
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

    pub async fn get_payment_method(
        &self,
        customer: &Customer,
    ) -> Result<Option<PaymentMethod>, BillingError> {
        let list_payment_methods = ListPaymentMethods {
            type_: Some(PaymentMethodTypeFilter::Card),
            customer: Some(customer.id.clone()),
            ..Default::default()
        };
        let mut payment_methods = PaymentMethod::list(&self.client, &list_payment_methods).await?;

        if let Some(payment_method) = payment_methods.data.pop() {
            if payment_methods.data.is_empty() {
                Ok(Some(payment_method))
            } else {
                Err(BillingError::MultiplePaymentMethods(
                    payment_method,
                    payment_methods.data,
                ))
            }
        } else {
            Ok(None)
        }
    }

    // WARNING: Use caution when calling this directly as multiple payment methods can be created
    // Use `get_or_create_payment_method` instead!
    async fn create_payment_method(
        &self,
        payment_card: PaymentCard,
    ) -> Result<PaymentMethod, BillingError> {
        let create_payment_method = CreatePaymentMethod {
            type_: Some(PaymentMethodTypeFilter::Card),
            card: Some(CreatePaymentMethodCardUnion::CardDetailsParams(
                payment_card,
            )),
            ..Default::default()
        };
        PaymentMethod::create(&self.client, create_payment_method)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    use super::PaymentCard;
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
        assert!(biller.get_customer(&email).await.unwrap().is_none());
        let create_customer = biller.create_customer(&name, &email).await.unwrap();
        let get_customer = biller.get_customer(&email).await.unwrap().unwrap();
        assert_eq!(create_customer.id, get_customer.id);
        let get_or_create_customer = biller.get_or_create_customer(&name, &email).await.unwrap();
        assert_eq!(create_customer.id, get_or_create_customer.id);

        let payment_card = PaymentCard {
            number: "4000008260000000".into(),
            exp_year: Utc::now().year() + 1,
            exp_month: 1,
            cvc: Some("123".to_string()),
        };
        let payment_method = biller
            .get_or_create_payment_method(&create_customer, payment_card)
            .await
            .unwrap();
    }
}
