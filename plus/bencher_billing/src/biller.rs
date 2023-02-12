use std::collections::HashMap;

use bencher_json::{
    system::config::{JsonBilling, JsonProduct, JsonProducts},
    Email, NonEmpty,
};
use stripe::{
    AttachPaymentMethod, Client, CreateCustomer, CreatePaymentMethod, CreatePaymentMethodCardUnion,
    ListCustomers, ListPaymentMethods, PaymentMethod, PaymentMethodTypeFilter, Price, Product,
};
pub use stripe::{CardDetailsParams as PaymentCard, Customer, Subscription};

use crate::BillingError;

pub struct Biller {
    client: Client,
    products: BillerProducts,
}

impl Biller {
    pub async fn new(billing: JsonBilling) -> Result<Self, BillingError> {
        let JsonBilling {
            secret_key,
            products,
        } = billing;
        let client = Client::new(secret_key);
        let products = BillerProducts::new(&client, products).await?;

        Ok(Self { client, products })
    }
}

pub struct BillerProducts {
    pub team: BillerProduct,
    pub enterprise: BillerProduct,
}

impl BillerProducts {
    async fn new(client: &Client, products: JsonProducts) -> Result<Self, BillingError> {
        let JsonProducts { team, enterprise } = products;

        Ok(Self {
            team: BillerProduct::new(client, team).await?,
            enterprise: BillerProduct::new(client, enterprise).await?,
        })
    }
}

pub struct BillerProduct {
    pub product: Product,
    pub pricing: HashMap<String, Price>,
}

impl BillerProduct {
    async fn new(client: &Client, product: JsonProduct) -> Result<Self, BillingError> {
        let JsonProduct { id, pricing } = product;
        let product = Product::retrieve(client, &id.parse()?, &[]).await?;

        let mut biller_pricing = HashMap::with_capacity(pricing.len());
        for (price_name, price_id) in pricing {
            let price = Price::retrieve(client, &price_id.parse()?, &[]).await?;
            biller_pricing.insert(price_name, price);
        }

        Ok(Self {
            product,
            pricing: biller_pricing,
        })
    }
}

impl Biller {
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
            self.create_payment_method(customer, payment_card).await
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
        customer: &Customer,
        payment_card: PaymentCard,
    ) -> Result<PaymentMethod, BillingError> {
        let create_payment_method = CreatePaymentMethod {
            type_: Some(PaymentMethodTypeFilter::Card),
            card: Some(CreatePaymentMethodCardUnion::CardDetailsParams(
                payment_card,
            )),
            ..Default::default()
        };
        let payment_method = PaymentMethod::create(&self.client, create_payment_method).await?;

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
    use bencher_json::system::config::{JsonBilling, JsonProduct, JsonProducts};
    use chrono::{Datelike, Utc};
    use literally::hmap;
    use pretty_assertions::assert_eq;

    use super::PaymentCard;
    use crate::Biller;

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn test_billing_key() -> Option<String> {
        std::env::var(TEST_BILLING_KEY).ok()
    }

    fn test_products() -> JsonProducts {
        JsonProducts {
            team: JsonProduct {
                id: "prod_NKz5B9dGhDiSY1".into(),
                pricing: hmap! {
                    "default".to_string() => "price_1MaJ7kKal5vzTlmh1pbQ5JYR".to_string(),
                },
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                pricing: hmap! {
                    "default".to_string() => "price_1MaViyKal5vzTlmho1MdXIpe".to_string(),
                },
            },
        }
    }

    #[tokio::test]
    async fn test_biller() {
        let Some(billing_key) = test_billing_key() else {
            return;
        };
        let json_billing = JsonBilling {
            secret_key: billing_key.parse().unwrap(),
            products: test_products(),
        };
        let biller = Biller::new(json_billing).await.unwrap();

        let name = "Muriel Bagge".parse().unwrap();
        let email = format!("muriel.bagge.{}@nowhere.com", rand::random::<u64>())
            .parse()
            .unwrap();
        assert!(biller.get_customer(&email).await.unwrap().is_none());
        let create_customer = biller.create_customer(&name, &email).await.unwrap();
        let get_customer = biller.get_customer(&email).await.unwrap().unwrap();
        assert_eq!(create_customer.id, get_customer.id);
        let customer = create_customer;
        let get_or_create_customer = biller.get_or_create_customer(&name, &email).await.unwrap();
        assert_eq!(customer.id, get_or_create_customer.id);

        let payment_card = PaymentCard {
            number: "4000008260000000".into(),
            exp_year: Utc::now().year() + 1,
            exp_month: 1,
            cvc: Some("123".to_string()),
        };
        assert!(biller
            .get_payment_method(&customer)
            .await
            .unwrap()
            .is_none());
        let create_payment_method = biller
            .create_payment_method(&customer, payment_card.clone())
            .await
            .unwrap();
        let get_payment_method = biller.get_payment_method(&customer).await.unwrap().unwrap();
        assert_eq!(create_payment_method.id, get_payment_method.id);
        let payment_method = create_payment_method;
        let get_or_create_payment_method = biller
            .get_or_create_payment_method(&customer, payment_card)
            .await
            .unwrap();
        assert_eq!(payment_method.id, get_or_create_payment_method.id);
    }
}
